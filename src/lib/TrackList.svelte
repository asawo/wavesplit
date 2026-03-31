<script>
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { open as openDialog } from '@tauri-apps/plugin-dialog'
  import { onMount, onDestroy } from 'svelte'

  let { tracks = $bindable([]), refresh = $bindable(null) } = $props()

  // pipeline events keyed by track id: { stage, status, message }
  let progress = $state({})

  let unlisten

  onMount(async () => {
    refresh = refreshTracks
    unlisten = await listen('pipeline', (event) => {
      const { track_id, stage, status, message } = event.payload
      progress = {
        ...progress,
        [track_id]: { stage, status, message },
      }
      // Refresh from DB whenever any stage finishes or errors
      if (status === 'done' || status === 'error') {
        refreshTracks()
      }
    })
    await refreshTracks()
  })

  onDestroy(() => unlisten?.())

  async function refreshTracks() {
    tracks = await invoke('list_tracks')
  }

  let exportingId = $state(null)

  async function exportStems(track) {
    const dest = await openDialog({ directory: true, title: 'Export stems to…' })
    if (!dest) return
    exportingId = track.id
    try {
      const files = await invoke('export_stems', { trackId: track.id, destDir: dest })
      exportingId = null
      alert(`Exported ${files.join(', ')} to ${dest}`)
    } catch (e) {
      exportingId = null
      alert(`Export failed: ${e}`)
    }
  }

  async function deleteTrack(id) {
    await invoke('delete_track', { id })
    tracks = tracks.filter((t) => t.id !== id)
    const { [id]: _, ...rest } = progress
    progress = rest
  }

  function statusLabel(track) {
    const p = progress[track.id]
    if (p) {
      if (p.status === 'error') return `Error: ${p.message ?? p.stage}`
      if (p.status === 'started') return stageLabel(p.stage)
      if (p.status === 'done' && p.stage !== 'analysis') return stageLabel(nextStage(p.stage))
    }
    if (track.status_analysis === 'done') return 'Ready'
    if (track.status_stems === 'error' || track.status_download === 'error' || track.status_analysis === 'error') {
      return `Error (${track.error_message ?? 'unknown'})`
    }
    if (track.status_analysis === 'pending' && track.status_stems === 'pending' && track.status_download === 'pending') {
      return 'Queued'
    }
    return 'Processing'
  }

  function stageLabel(stage) {
    return { download: 'Downloading…', stems: 'Separating stems…', analysis: 'Analyzing…' }[stage] ?? stage
  }

  function nextStage(stage) {
    return { download: 'stems', stems: 'analysis' }[stage]
  }

  function isReady(track) {
    return track.status_analysis === 'done'
  }

  function hasError(track) {
    const p = progress[track.id]
    if (p?.status === 'error') return true
    return track.status_download === 'error' || track.status_stems === 'error' || track.status_analysis === 'error'
  }

  function isProcessing(track) {
    return !isReady(track) && !hasError(track)
  }

  const STAGE_PROGRESS = { download: 15, stems: 50, analysis: 85 }

  function progressPct(track) {
    if (isReady(track)) return 100
    const p = progress[track.id]
    if (!p) return 0
    const base = STAGE_PROGRESS[p.stage] ?? 0
    return p.status === 'done' ? base + 15 : base
  }
</script>

<div class="track-list">
  {#if tracks.length === 0}
    <p class="empty">No tracks yet. Add a YouTube URL or open a local file.</p>
  {/if}

  {#each tracks as track (track.id)}
    <div class="track" class:ready={isReady(track)} class:error={hasError(track)}>
      <div class="track-info">
        <span class="title">{track.title}</span>
        <span class="status" class:processing={isProcessing(track)}>
          {statusLabel(track)}
        </span>
        {#if isProcessing(track)}
          <div class="progress-bar">
            <div class="progress-fill" style="width: {progressPct(track)}%"></div>
          </div>
        {/if}
      </div>
      <div class="track-actions">
        {#if isReady(track)}
          <button class="export-btn" onclick={() => exportStems(track)} disabled={exportingId === track.id}>
            {exportingId === track.id ? 'Exporting…' : '↓ Export stems'}
          </button>
        {/if}
        <button class="delete-btn" onclick={() => deleteTrack(track.id)} title="Delete track">✕</button>
      </div>
    </div>
  {/each}
</div>

<style>
  .track-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .empty {
    color: var(--fg-muted);
    font-size: 13px;
    text-align: center;
    padding: 32px 0;
    margin: 0;
  }

  .track {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px;
    border-radius: 6px;
    background: var(--bg-track);
    gap: 12px;
  }

  .track.ready {
    background: var(--bg-track-ready);
  }

  .track.error {
    background: var(--bg-track-error);
  }

  .track-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .title {
    font-size: 14px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .status {
    font-size: 11px;
    color: var(--fg-muted);
  }

  .status.processing {
    color: var(--color-processing);
  }

  .progress-bar {
    height: 3px;
    background: var(--border);
    border-radius: 2px;
    overflow: hidden;
    margin-top: 5px;
    width: 100%;
    max-width: 240px;
  }

  .progress-fill {
    height: 100%;
    background: var(--color-processing);
    border-radius: 2px;
    transition: width 0.4s ease;
  }

  .track-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .export-btn {
    padding: 4px 12px;
    border: 1px solid var(--accent);
    border-radius: 4px;
    background: transparent;
    color: var(--accent);
    font-size: 12px;
    cursor: pointer;
    white-space: nowrap;
  }

  .export-btn:hover:not(:disabled) {
    background: var(--accent);
    color: #fff;
  }

  .export-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .delete-btn {
    padding: 4px 8px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
    font-size: 13px;
    cursor: pointer;
    line-height: 1;
  }

  .delete-btn:hover {
    color: var(--color-error);
    background: var(--bg-track-error);
  }
</style>
