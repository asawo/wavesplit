<script>
  import { invoke } from '@tauri-apps/api/core'
  import { open as openDialog } from '@tauri-apps/plugin-dialog'

  let { track, active, onBack } = $props()

  const STEMS = [
    { key: 'vocals', label: 'Vocals', color: '#4caf72' },
    { key: 'drums',  label: 'Drums',  color: '#4a9eff' },
    { key: 'bass',   label: 'Bass',   color: '#f0a030' },
    { key: 'other',  label: 'Other',  color: '#e06080' },
  ]

  let playing = $state(false)
  let playhead = $state(0)

  $effect(() => {
    if (!active) playing = false
  })

  let stemState = $state(
    Object.fromEntries(STEMS.map(s => [s.key, { muted: false, soloed: false, volume: 1 }]))
  )

  function toggleMute(key) {
    stemState[key] = { ...stemState[key], muted: !stemState[key].muted }
  }

  function toggleSolo(key) {
    const wasSoloed = stemState[key].soloed
    for (const k of Object.keys(stemState)) {
      stemState[k] = { ...stemState[k], soloed: false }
    }
    if (!wasSoloed) {
      stemState[key] = { ...stemState[key], soloed: true }
    }
  }

  let anySoloed = $derived(Object.values(stemState).some(s => s.soloed))

  function isMuted(key) {
    if (anySoloed) return !stemState[key].soloed
    return stemState[key].muted
  }

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

  function formatTime(ms) {
    if (!ms && ms !== 0) return '--:--'
    const s = Math.floor(ms / 1000)
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
  }

  function seekTo(e) {
    const rect = e.currentTarget.getBoundingClientRect()
    playhead = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width))
  }

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
  <header class="playback-header">
    <button class="back-btn" onclick={onBack}>‹ Library</button>
    <div class="track-meta">
      <span class="track-title">{track.title}</span>
      {#if track.artist}<span class="track-artist">{track.artist}</span>{/if}
    </div>
    <div class="header-spacer"></div>
  </header>

  <div class="master-section">
    <p class="section-label">Master</p>
    <div class="master-panel">
      <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions a11y_no_noninteractive_element_interactions -->
      <div class="waveform-wrap" role="presentation" onclick={seekTo}>
        <svg class="waveform" viewBox="0 0 400 60" preserveAspectRatio="none">
          {#each makeWaveformBars(track.id, 120) as h, i}
            {@const x = i * (400 / 120)}
            {@const bh = h * 54}
            <rect
              x={x} y={(60 - bh) / 2}
              width="2.2" height={bh} rx="1"
              fill={(i / 120) < playhead ? '#4caf72' : '#383838'}
            />
          {/each}
        </svg>
        <div class="playhead" style="left:{playhead * 100}%">
          <div class="playhead-dot"></div>
        </div>
      </div>
      <div class="time-row">
        <span>{formatTime(playhead * (track.duration_ms ?? 0))}</span>
        <span>{formatTime(track.duration_ms)}</span>
      </div>
    </div>
  </div>

  <div class="transport">
    <button class="transport-btn" title="Skip to start" onclick={() => playhead = 0}>‹</button>
    <button class="transport-btn" title="Rewind">‹‹</button>
    <button class="transport-btn play-btn" title={playing ? 'Pause' : 'Play'} onclick={() => playing = !playing}>
      {playing ? '⏸' : '▶'}
    </button>
    <button class="transport-btn" title="Fast forward">››</button>
    <button class="transport-btn" title="Skip to end" onclick={() => playhead = 1}>›</button>
  </div>

  <div class="stems-section">
    <p class="section-label">Stems</p>
    {#each STEMS as stem}
      {@const state = stemState[stem.key]}
      {@const muted = isMuted(stem.key)}
      <div class="stem-row">
        <span class="stem-label" style="color:{muted ? 'var(--fg-muted)' : stem.color}">{stem.label}</span>
        <div class="stem-waveform-wrap">
          <svg class="stem-waveform" viewBox="0 0 400 28" preserveAspectRatio="none">
            {#each makeWaveformBars(track.id + stem.key, 120) as h, i}
              {@const x = i * (400 / 120)}
              {@const bh = h * 24}
              <rect
                x={x} y={(28 - bh) / 2}
                width="2.2" height={bh} rx="0.5"
                fill={muted ? '#2e2e2e' : ((i / 120) < playhead ? stem.color : '#383838')}
                opacity={muted ? 0.5 : 1}
              />
            {/each}
          </svg>
          <div class="stem-playhead" style="left:{playhead * 100}%"></div>
        </div>
        <button
          class="stem-btn"
          class:active={state.muted && !anySoloed}
          onclick={() => toggleMute(stem.key)}
          title="Mute"
        >M</button>
        <button
          class="stem-btn"
          class:active={state.soloed}
          onclick={() => toggleSolo(stem.key)}
          title="Solo"
        >S</button>
        <input
          class="vol-slider"
          type="range"
          min="0" max="1" step="0.01"
          value={state.volume}
          style="accent-color:{stem.color}"
          oninput={(e) => stemState[stem.key] = { ...state, volume: +e.target.value }}
        />
      </div>
    {/each}
  </div>

  <div class="playback-footer">
    {#if exportError}
      <span class="export-error">{exportError} <button class="dismiss-btn" onclick={() => exportError = ''}>×</button></span>
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

  .header-spacer {
    /* balances the back button */
  }

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

  .transport-btn:hover {
    background: var(--bg-button-hover);
    border-color: var(--fg-muted);
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
