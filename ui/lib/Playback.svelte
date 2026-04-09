<script>
  import { invoke, convertFileSrc } from '@tauri-apps/api/core'
  import { open as openDialog } from '@tauri-apps/plugin-dialog'
  import { onDestroy } from 'svelte'

  let { track, active, onBack } = $props()

  const STEMS = [
    { key: 'vocals', label: 'Vocals', color: '#4caf72' },
    { key: 'drums',  label: 'Drums',  color: '#4a9eff' },
    { key: 'bass',   label: 'Bass',   color: '#f0a030' },
    { key: 'other',  label: 'Other',  color: '#e06080' },
  ]

  // ── Transport ──────────────────────────────────────────────
  let playing = $state(false)
  let playhead = $state(0)   // 0–1 fraction

  // ── Stem mixer ─────────────────────────────────────────────
  let stemState = $state(
    Object.fromEntries(STEMS.map(s => [s.key, { muted: false, soloed: false, volume: 1 }]))
  )

  function toggleMute(key) {
    stemState[key] = { ...stemState[key], muted: !stemState[key].muted }
  }

  function toggleSolo(key) {
    const wasSoloed = stemState[key].soloed
    for (const k of Object.keys(stemState)) stemState[k] = { ...stemState[k], soloed: false }
    if (!wasSoloed) stemState[key] = { ...stemState[key], soloed: true }
  }

  let anySoloed = $derived(Object.values(stemState).some(s => s.soloed))

  function isMuted(key) {
    return anySoloed ? !stemState[key].soloed : stemState[key].muted
  }

  // ── Audio engine ───────────────────────────────────────────
  let audioCtx = null
  let gainNodes = {}       // key → GainNode
  let sourceNodes = {}     // key → AudioBufferSourceNode (live while playing)
  let buffers = $state({}) // key → AudioBuffer
  let waveformData = $state({}) // key → number[120] (RMS, 0–1 normalised)
  let loading = $state(false)
  let loadError = $state(null)
  let duration = $state(0) // seconds (from decoded audio)
  let startOffset = 0      // track pos (s) where last play() started from
  let startTime = 0        // audioCtx.currentTime when last play() started
  let rafId = null
  let loadedTrackId = null

  // Time display — prefer real decoded duration, fall back to DB
  let displayDuration = $derived(duration > 0 ? duration : (track.duration_ms ?? 0) / 1000)
  let elapsedSeconds = $derived(playhead * displayDuration)

  function formatTime(s) {
    if (!s && s !== 0) return '--:--'
    s = Math.floor(s)
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
  }

  // Deterministic fallback waveform while loading
  function hashStr(str) {
    let h = 0
    for (const c of str) h = (Math.imul(31, h) + c.charCodeAt(0)) | 0
    return h >>> 0
  }

  function makeWaveformBars(seed, count) {
    let s = hashStr(seed)
    return Array.from({ length: count }, () => {
      s = (Math.imul(s, 1664525) + 1013904223) >>> 0
      return 0.12 + (s / 0xFFFFFFFF) * 0.88
    })
  }

  function extractWaveform(audioBuffer, numPoints) {
    const channel = audioBuffer.getChannelData(0)
    const blockSize = Math.floor(channel.length / numPoints)
    const result = new Array(numPoints)
    for (let i = 0; i < numPoints; i++) {
      const start = i * blockSize
      let sum = 0
      for (let j = start; j < Math.min(start + blockSize, channel.length); j++) {
        sum += channel[j] * channel[j]
      }
      result[i] = Math.sqrt(sum / Math.max(blockSize, 1))
    }
    const max = Math.max(...result, 0.001)
    return result.map(v => v / max)
  }

  // Master waveform = RMS average of all loaded stems
  let masterWaveform = $derived((() => {
    const loaded = STEMS.map(s => waveformData[s.key]).filter(Boolean)
    if (!loaded.length) return null
    const avg = new Array(120).fill(0)
    for (const w of loaded) {
      for (let i = 0; i < 120; i++) avg[i] += w[i] / loaded.length
    }
    return avg
  })())

  function applyGains() {
    for (const stem of STEMS) {
      // Read reactive state first so $effect always tracks these as dependencies,
      // even before gain nodes are created (early-return would skip the reads).
      const s = stemState[stem.key]
      const muted = anySoloed ? !s.soloed : s.muted
      const target = muted ? 0 : s.volume
      const node = gainNodes[stem.key]
      if (!node) continue
      if (audioCtx) {
        node.gain.setTargetAtTime(target, audioCtx.currentTime, 0.015)
      } else {
        node.gain.value = target
      }
    }
  }

  async function loadAudio() {
    const targetId = track.id
    if (loadedTrackId === targetId) return
    loadedTrackId = targetId

    loading = true
    loadError = null
    playing = false
    startOffset = 0
    playhead = 0
    buffers = {}
    waveformData = {}

    try {
      if (!audioCtx) {
        audioCtx = new AudioContext()
        for (const stem of STEMS) {
          gainNodes[stem.key] = audioCtx.createGain()
          gainNodes[stem.key].connect(audioCtx.destination)
        }
        applyGains()
      }

      const paths = await invoke('get_stem_paths', { trackId: targetId })

      const results = await Promise.all(STEMS.map(async ({ key }) => {
        const url = convertFileSrc(paths[key])
        const resp = await fetch(url)
        if (!resp.ok) throw new Error(`HTTP ${resp.status} for ${key}`)
        const ab = await resp.arrayBuffer()
        const buf = await audioCtx.decodeAudioData(ab)
        return [key, buf]
      }))

      // Discard results if the user switched tracks while we were loading
      if (loadedTrackId !== targetId) return

      const newBuffers = Object.fromEntries(results)
      buffers = newBuffers
      waveformData = Object.fromEntries(
        results.map(([key, buf]) => [key, extractWaveform(buf, 120)])
      )
      duration = Object.values(newBuffers)[0]?.duration ?? 0

    } catch (e) {
      if (loadedTrackId === targetId) {
        loadError = e.message ?? String(e)
        loadedTrackId = null  // allow retry on next open
      }
    } finally {
      if (loadedTrackId === targetId || loadedTrackId === null) {
        loading = false
      }
    }
  }

  // ── Playback control ───────────────────────────────────────

  function startPlayback() {
    if (!audioCtx || Object.keys(buffers).length === 0) return
    const offset = Math.max(0, Math.min(startOffset, duration - 0.01))
    startTime = audioCtx.currentTime
    for (const { key } of STEMS) {
      const buf = buffers[key]
      if (!buf) continue
      const src = audioCtx.createBufferSource()
      src.buffer = buf
      src.connect(gainNodes[key])
      src.start(0, offset)
      sourceNodes[key] = src
    }
    schedTick()
  }

  function stopSources() {
    for (const src of Object.values(sourceNodes)) {
      try { src.stop() } catch (_) {}
      try { src.disconnect() } catch (_) {}
    }
    sourceNodes = {}
  }

  function pausePlayback() {
    if (audioCtx && Object.keys(sourceNodes).length > 0) {
      startOffset = Math.min(startOffset + (audioCtx.currentTime - startTime), duration)
    }
    stopSources()
    cancelTick()
  }

  async function handlePlayPause() {
    if (!audioCtx || Object.keys(buffers).length === 0) return
    if (playing) {
      pausePlayback()
      playing = false
    } else {
      if (audioCtx.state === 'suspended') await audioCtx.resume()
      startPlayback()
      playing = true
    }
  }

  async function seek(fraction) {
    const safeFraction = Math.max(0, Math.min(1, fraction))
    const was = playing
    if (was) { stopSources(); cancelTick(); playing = false }
    startOffset = safeFraction * duration
    playhead = safeFraction
    if (was) {
      if (audioCtx?.state === 'suspended') await audioCtx.resume()
      startPlayback()
      playing = true
    }
  }

  function seekToClick(e) {
    const rect = e.currentTarget.getBoundingClientRect()
    seek((e.clientX - rect.left) / rect.width)
  }

  function getCurrentPos() {
    if (!playing || !audioCtx) return startOffset
    return startOffset + (audioCtx.currentTime - startTime)
  }

  function skipBy(seconds) {
    seek((getCurrentPos() + seconds) / Math.max(duration, 0.001))
  }

  function schedTick() {
    cancelTick()
    rafId = requestAnimationFrame(tick)
  }

  function cancelTick() {
    if (rafId) { cancelAnimationFrame(rafId); rafId = null }
  }

  function tick() {
    if (!playing || !audioCtx) return
    const pos = startOffset + (audioCtx.currentTime - startTime)
    if (pos >= duration) {
      stopSources()
      playing = false
      startOffset = 0
      playhead = 0
      return
    }
    playhead = pos / duration
    rafId = requestAnimationFrame(tick)
  }

  // Sync gain nodes whenever stem state or solo changes
  $effect(() => {
    applyGains()
  })

  // Pause when navigating back to library
  $effect(() => {
    if (!active && playing) { pausePlayback(); playing = false }
  })

  // Reload whenever the selected track changes
  $effect(() => {
    const id = track.id  // reactive — re-runs when track changes
    if (id !== loadedTrackId) loadAudio()
  })

  onDestroy(() => {
    cancelTick()
    stopSources()
    audioCtx?.close()
  })

  // ── Export ─────────────────────────────────────────────────
  let exportingId = $state(null)
  let exportError = $state('')

  async function exportStems() {
    const dest = await openDialog({ directory: true, title: 'Export stems to…' })
    if (!dest) return
    exportingId = track.id
    exportError = ''
    try {
      await invoke('export_stems', { trackId: track.id, destDir: dest })
    } catch (e) {
      exportError = String(e)
    } finally {
      exportingId = null
    }
  }
</script>

<div class="playback">
  <!-- ── Header ── -->
  <header class="playback-header">
    <button class="back-btn" onclick={onBack}>‹ Library</button>
    <div class="track-meta">
      <span class="track-title">{track.title}</span>
      {#if track.artist}<span class="track-artist">{track.artist}</span>{/if}
    </div>
    <div class="header-spacer"></div>
  </header>

  <!-- ── Master waveform ── -->
  <div class="master-section">
    <p class="section-label">Master</p>
    <div class="master-panel">
      <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions a11y_no_noninteractive_element_interactions -->
      <div class="waveform-wrap" role="presentation" onclick={seekToClick}
           style="opacity:{loading ? 0.4 : 1}; transition:opacity 0.3s">
        <svg class="waveform" viewBox="0 0 400 60" preserveAspectRatio="none">
          {#each (masterWaveform ?? makeWaveformBars(track.id, 120)) as h, i}
            {@const x = i * (400 / 120)}
            {@const bh = h * 54}
            <rect x={x} y={(60 - bh) / 2} width="2.2" height={bh} rx="1"
                  fill={(i / 120) < playhead ? '#4caf72' : '#383838'} />
          {/each}
        </svg>
        <div class="playhead" style="left:{playhead * 100}%">
          <div class="playhead-dot"></div>
        </div>
      </div>
      <div class="time-row">
        <span>{formatTime(elapsedSeconds)}</span>
        <span>{formatTime(displayDuration)}</span>
      </div>
    </div>
  </div>

  <!-- ── Transport ── -->
  <div class="transport">
    <button class="transport-btn" title="Skip to start" onclick={() => seek(0)}>‹</button>
    <button class="transport-btn" title="Rewind 10s"    onclick={() => skipBy(-10)}>‹‹</button>
    <button class="transport-btn play-btn"
            title={playing ? 'Pause' : 'Play'}
            disabled={loading || !!loadError}
            onclick={handlePlayPause}>
      {playing ? '⏸' : '▶'}
    </button>
    <button class="transport-btn" title="Forward 10s"  onclick={() => skipBy(10)}>››</button>
    <button class="transport-btn" title="Skip to end"  onclick={() => seek(1)}>›</button>
  </div>

  <!-- ── Stems ── -->
  <div class="stems-section">
    <p class="section-label">Stems</p>

    {#if loadError}
      <p class="load-error">Failed to load audio: {loadError}</p>
    {/if}

    {#each STEMS as stem}
      {@const state = stemState[stem.key]}
      {@const muted = isMuted(stem.key)}
      <div class="stem-row">
        <span class="stem-label" style="color:{muted ? 'var(--fg-muted)' : stem.color}">{stem.label}</span>
        <div class="stem-waveform-wrap" style="opacity:{loading ? 0.35 : 1}; transition:opacity 0.3s">
          <svg class="stem-waveform" viewBox="0 0 400 28" preserveAspectRatio="none">
            {#each (waveformData[stem.key] ?? makeWaveformBars(track.id + stem.key, 120)) as h, i}
              {@const x = i * (400 / 120)}
              {@const bh = h * 24}
              <rect x={x} y={(28 - bh) / 2} width="2.2" height={bh} rx="0.5"
                    fill={muted ? '#2e2e2e' : ((i / 120) < playhead ? stem.color : '#383838')}
                    opacity={muted ? 0.5 : 1} />
            {/each}
          </svg>
          <div class="stem-playhead" style="left:{playhead * 100}%"></div>
        </div>
        <button class="stem-btn" class:active={state.muted && !anySoloed}
                onclick={() => toggleMute(stem.key)} title="Mute">M</button>
        <button class="stem-btn" class:active={state.soloed}
                onclick={() => toggleSolo(stem.key)} title="Solo">S</button>
        <input class="vol-slider" type="range" min="0" max="1" step="0.01"
               value={state.volume}
               style="accent-color:{stem.color}"
               oninput={(e) => stemState[stem.key] = { ...state, volume: +e.target.value }} />
      </div>
    {/each}
  </div>

  <!-- ── Footer ── -->
  <div class="playback-footer">
    {#if exportError}
      <span class="export-error">{exportError}
        <button class="dismiss-btn" onclick={() => exportError = ''}>×</button>
      </span>
    {/if}
    <button class="export-btn" onclick={exportStems} disabled={!!exportingId}>
      {exportingId ? 'Exporting…' : '↓ Export stems'}
    </button>
  </div>
</div>

<style>
  .playback {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg);
    overflow: hidden;
  }

  /* ── Header ── */
  .playback-header {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    padding: 10px 16px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    gap: 12px;
  }

  .back-btn {
    justify-self: start;
    background: transparent;
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--fg);
    font-size: 13px;
    padding: 5px 14px;
    cursor: pointer;
    white-space: nowrap;
  }

  .back-btn:hover {
    background: var(--bg-button-hover);
    border-color: var(--fg-muted);
  }

  .track-meta {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    min-width: 0;
  }

  .track-title {
    font-size: 15px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 260px;
  }

  .track-artist {
    font-size: 12px;
    color: var(--fg-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 260px;
  }

  .header-spacer { /* balances back button */ }

  /* ── Master waveform ── */
  .master-section {
    padding: 10px 16px 0;
    flex-shrink: 0;
  }

  .section-label {
    margin: 0 0 6px;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--fg-muted);
  }

  .master-panel {
    background: #141414;
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 10px 12px 8px;
  }

  .waveform-wrap {
    position: relative;
    height: 60px;
    cursor: crosshair;
    user-select: none;
  }

  .waveform {
    width: 100%;
    height: 100%;
    display: block;
  }

  .playhead {
    position: absolute;
    inset: 0 auto;
    width: 1.5px;
    background: rgba(255, 255, 255, 0.85);
    transform: translateX(-50%);
    pointer-events: none;
  }

  .playhead-dot {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #4caf72;
    border: 2px solid #fff;
  }

  .time-row {
    display: flex;
    justify-content: space-between;
    margin-top: 6px;
    font-size: 11px;
    color: var(--fg-muted);
    font-variant-numeric: tabular-nums;
  }

  /* ── Transport ── */
  .transport {
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 8px;
    padding: 10px 16px;
    flex-shrink: 0;
    border-bottom: 1px solid var(--border);
  }

  .transport-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 40px;
    height: 36px;
    padding: 0 14px;
    border: 1px solid #444;
    border-radius: 4px;
    background: transparent;
    color: var(--fg);
    font-size: 14px;
    cursor: pointer;
    line-height: 1;
    letter-spacing: -1px;
  }

  .transport-btn:hover:not(:disabled) {
    background: var(--bg-button-hover);
    border-color: var(--fg-muted);
  }

  .transport-btn:disabled {
    opacity: 0.35;
    cursor: default;
  }

  .play-btn {
    min-width: 44px;
    font-size: 16px;
    letter-spacing: 0;
  }

  /* ── Stems ── */
  .stems-section {
    flex: 1;
    overflow-y: auto;
    padding: 10px 16px;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .stems-section .section-label {
    margin-bottom: 4px;
    flex-shrink: 0;
  }

  .load-error {
    font-size: 12px;
    color: var(--color-error);
    margin: 0 0 8px;
  }

  .stem-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .stem-row:last-child {
    border-bottom: none;
  }

  .stem-label {
    font-size: 13px;
    font-weight: 500;
    width: 52px;
    flex-shrink: 0;
    transition: color 0.15s;
  }

  .stem-waveform-wrap {
    flex: 1;
    position: relative;
    height: 28px;
    min-width: 0;
  }

  .stem-waveform {
    width: 100%;
    height: 100%;
    display: block;
  }

  .stem-playhead {
    position: absolute;
    inset: 0 auto;
    width: 1px;
    background: rgba(255, 255, 255, 0.35);
    transform: translateX(-50%);
    pointer-events: none;
  }

  .stem-btn {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--fg-muted);
    font-size: 11px;
    font-weight: 700;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    line-height: 1;
  }

  .stem-btn:hover {
    border-color: var(--fg-muted);
    color: var(--fg);
  }

  .stem-btn.active {
    background: var(--fg);
    border-color: var(--fg);
    color: var(--bg);
  }

  .vol-slider {
    width: 88px;
    flex-shrink: 0;
    cursor: pointer;
  }

  /* ── Footer ── */
  .playback-footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 12px;
    padding: 10px 16px;
    border-top: 1px solid var(--border);
    flex-shrink: 0;
  }

  .export-error {
    font-size: 12px;
    color: var(--color-error);
  }

  .dismiss-btn {
    background: none;
    border: none;
    color: var(--color-error);
    cursor: pointer;
    font-size: 13px;
    padding: 0 0 0 4px;
  }

  .export-btn {
    padding: 7px 18px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: transparent;
    color: var(--fg);
    font-size: 13px;
    cursor: pointer;
  }

  .export-btn:hover:not(:disabled) {
    background: var(--bg-button-hover);
    border-color: var(--fg-muted);
  }

  .export-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }
</style>
