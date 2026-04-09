<script>
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { open as openDialog, confirm } from '@tauri-apps/plugin-dialog'

  import { onMount, onDestroy } from 'svelte'
  import { fuzzy, SORT_FNS, isReady, hasError, statusLabel, progressPct } from './tracklist.helpers.js'

  let { tracks = $bindable([]), refresh = $bindable(null), onPlay } = $props()

  // pipeline events keyed by track id: { stage, status, message }
  let progress = $state({})

  let filterQuery = $state('')
  let sortKey = $state('newest')

  function matchesFilter(track) {
    if (!filterQuery) return true
    return fuzzy(filterQuery, track.title) || fuzzy(filterQuery, track.artist ?? '')
  }

  let displayTracks = $derived(
    tracks.filter(matchesFilter).sort(SORT_FNS[sortKey])
  )

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

  const PENDING_ID = '__pending__'

  async function refreshTracks() {
    tracks = await invoke('list_tracks')
  }

  // Inline editing: editingId = track id currently being edited
  let editingId = $state(null)
  let editTitle = $state('')
  let editArtist = $state('')

  function startEdit(track) {
    editingId = track.id
    editTitle = track.title
    editArtist = track.artist ?? ''
  }

  async function commitEdit(track) {
    if (editingId !== track.id) return
    editingId = null
    const trimmedTitle = editTitle.trim() || track.title
    const trimmedArtist = editArtist.trim() || null
    if (trimmedTitle === track.title && trimmedArtist === (track.artist ?? null)) return
    editError = ''
    try {
      await invoke('update_track_meta', { id: track.id, title: trimmedTitle, artist: trimmedArtist })
      await refreshTracks()
    } catch (e) {
      editError = String(e)
      await refreshTracks()
    }
  }

  function onEditKeydown(e, track) {
    if (e.key === 'Enter') { e.target.blur() }
    if (e.key === 'Escape') { editingId = null }
  }

  let editError = $state('')

  let exportingId = $state(null)
  let exportError = $state('')

  async function exportStems(track) {
    const dest = await openDialog({ directory: true, title: 'Export stems to…' })
    if (!dest) return
    exportingId = track.id
    exportError = ''
    try {
      await invoke('export_stems', { trackId: track.id, destDir: dest })
      await refreshTracks()
    } catch (e) {
      exportError = String(e)
    } finally {
      exportingId = null
    }
  }

  let deleteError = $state('')

  let retryingId = $state(null)
  let retryError = $state('')

  async function retryTrack(track) {
    retryingId = track.id
    retryError = ''
    try {
      await invoke('retry_track', { id: track.id })
      await refreshTracks()
    } catch (e) {
      retryError = String(e)
    } finally {
      retryingId = null
    }
  }

  async function deleteTrack(track) {
    const ok = await confirm(
      `"${track.title}" and all its stems will be permanently deleted.`,
      { title: 'Delete track?', kind: 'warning', okLabel: 'Delete', cancelLabel: 'Cancel' }
    )
    if (!ok) return
    deleteError = ''
    try {
      await invoke('delete_track', { id: track.id })
      tracks = tracks.filter((t) => t.id !== track.id)
      const { [track.id]: _, ...rest } = progress
      progress = rest
    } catch (e) {
      deleteError = String(e)
    }
  }

  async function openFolder(path) {
    try {
      await invoke('open_folder', { path })
    } catch (e) {
      deleteError = String(e)
    }
  }

  function isProcessing(track) {
    return !isReady(track) && !hasError(track, progress)
  }
</script>

<div class="track-list">
  {#if tracks.length > 0}
    <div class="toolbar">
      <input class="filter-input" placeholder="Search" bind:value={filterQuery} />
      <select class="sort-select" bind:value={sortKey}>
        <option value="newest">Newest</option>
        <option value="oldest">Oldest</option>
        <option value="title">Title</option>
        <option value="artist">Artist</option>
      </select>
    </div>
  {/if}

  {#if editError}
    <p class="export-error">{editError} <button class="dismiss-error" onclick={() => editError = ''}>×</button></p>
  {/if}

  {#if deleteError}
    <p class="export-error">{deleteError} <button class="dismiss-error" onclick={() => deleteError = ''}>×</button></p>
  {/if}

  {#if exportError}
    <p class="export-error">{exportError} <button class="dismiss-error" onclick={() => exportError = ''}>×</button></p>
  {/if}

  {#if retryError}
    <p class="export-error">{retryError} <button class="dismiss-error" onclick={() => retryError = ''}>×</button></p>
  {/if}

  {#if tracks.length === 0}
    <p class="empty">No tracks yet. Add a YouTube URL or open a local file.</p>
  {/if}

  {#each displayTracks as track (track.id)}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div
      class="track"
      class:ready={isReady(track)}
      class:error={hasError(track, progress)}
      class:pending={track.id === PENDING_ID}
      class:playable={isReady(track) && !!onPlay}
      role={isReady(track) && onPlay ? 'button' : undefined}
      tabindex={isReady(track) && onPlay ? 0 : undefined}
      onclick={() => { if (onPlay && isReady(track) && editingId !== track.id) onPlay(track) }}
      onkeydown={(e) => { if (e.key === 'Enter' && onPlay && isReady(track) && editingId !== track.id) onPlay(track) }}
    >
      <div class="track-info">
        {#if track.id === PENDING_ID}
          <span class="title">{track.title}</span>
          <span class="status processing">Adding…</span>
        {:else if editingId === track.id}
          <input
            class="edit-input title-input"
            bind:value={editTitle}
            onblur={() => commitEdit(track)}
            onkeydown={(e) => onEditKeydown(e, track)}
          />
          <input
            class="edit-input artist-input"
            placeholder="Artist"
            bind:value={editArtist}
            onblur={() => commitEdit(track)}
            onkeydown={(e) => onEditKeydown(e, track)}
          />
        {:else}
          <span class="title" onclick={(e) => { e.stopPropagation(); startEdit(track) }} onkeydown={(e) => { e.stopPropagation(); e.key === 'Enter' && startEdit(track) }} role="button" tabindex="0">
            {track.title}
          </span>
          <span class="artist" onclick={(e) => { e.stopPropagation(); startEdit(track) }} onkeydown={(e) => { e.stopPropagation(); e.key === 'Enter' && startEdit(track) }} role="button" tabindex="0">
            {track.artist ?? '—'}
          </span>
        {/if}
        {#if track.id !== PENDING_ID}
          <span class="status" class:processing={isProcessing(track)} class:ready={isReady(track)}>
            {statusLabel(track, progress)}
          </span>
          {#if isProcessing(track)}
            <div class="progress-bar">
              <div class="progress-fill" style="width: {progressPct(track, progress)}%"></div>
            </div>
          {/if}
        {/if}
      </div>
      <div class="track-actions">
        {#if track.id === PENDING_ID}
          <span class="spinner" aria-label="Adding track"></span>
        {:else}
          {#if isReady(track)}
            {#if track.export_path}
              <button class="open-btn" onclick={() => openFolder(track.export_path)} title={track.export_path}>
                Open folder
              </button>
            {/if}
            <button class="export-btn" onclick={() => exportStems(track)} disabled={exportingId === track.id}>
              {exportingId === track.id ? 'Exporting…' : '↓ Export stems'}
            </button>
          {/if}
          {#if hasError(track, progress)}
            <button class="retry-btn" onclick={() => retryTrack(track)} disabled={retryingId === track.id}>
              {retryingId === track.id ? 'Retrying…' : '↺ Retry'}
            </button>
          {/if}
          <button class="delete-btn" onclick={() => deleteTrack(track)} title="Delete track">✕</button>
        {/if}
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

  .toolbar {
    display: flex;
    gap: 6px;
    margin-bottom: 6px;
  }

  .filter-input {
    flex: 1;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 5px;
    color: var(--fg);
    padding: 6px 10px;
    font-size: 13px;
    outline: none;
  }

  .filter-input:focus {
    border-color: var(--accent);
  }

  .filter-input::placeholder {
    color: var(--fg-muted);
  }

  .sort-select {
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 5px;
    color: var(--fg);
    padding: 6px 8px;
    font-size: 13px;
    outline: none;
    cursor: pointer;
    flex-shrink: 0;
  }

  .sort-select:focus {
    border-color: var(--accent);
  }

  .export-error {
    font-size: 12px;
    color: var(--color-error);
    margin: 0 0 6px;
  }

  .dismiss-error {
    background: none;
    border: none;
    color: var(--color-error);
    cursor: pointer;
    font-size: 14px;
    padding: 0 0 0 4px;
    line-height: 1;
    opacity: 0.7;
  }

  .dismiss-error:hover {
    opacity: 1;
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

  .track.error {
    background: var(--bg-track-error);
  }

  .track.playable {
    cursor: pointer;
  }

  .track.playable:hover {
    background: #2a312a;
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
    cursor: text;
  }

  .artist {
    font-size: 12px;
    color: var(--fg-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: text;
  }

  .edit-input {
    background: var(--bg-input);
    border: 1px solid var(--accent);
    border-radius: 3px;
    color: var(--fg);
    padding: 1px 6px;
    outline: none;
    width: 100%;
  }

  .title-input {
    font-size: 14px;
    font-weight: 500;
  }

  .artist-input {
    font-size: 12px;
  }

  .status {
    font-size: 11px;
    color: var(--fg-muted);
  }

  .status.ready {
    color: var(--color-ready);
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

  .open-btn {
    padding: 4px 10px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: transparent;
    color: var(--fg);
    font-size: 12px;
    cursor: pointer;
    white-space: nowrap;
  }

  .open-btn:hover {
    border-color: var(--fg-muted);
    background: var(--bg-button-hover);
  }

  .retry-btn {
    padding: 4px 10px;
    border: 1px solid var(--color-error);
    border-radius: 4px;
    background: transparent;
    color: var(--color-error);
    font-size: 12px;
    cursor: pointer;
    white-space: nowrap;
  }

  .retry-btn:hover:not(:disabled) {
    background: var(--color-error);
    color: #fff;
  }

  .retry-btn:disabled {
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

  .track.pending {
    opacity: 0.7;
  }

  .spinner {
    display: inline-block;
    width: 14px;
    height: 14px;
    border: 2px solid var(--border);
    border-top-color: var(--color-processing);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
    flex-shrink: 0;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
