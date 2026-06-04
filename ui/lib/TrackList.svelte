<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { open as openDialog, confirm } from "@tauri-apps/plugin-dialog";

  import { onMount, onDestroy } from "svelte";
  import {
    fuzzy,
    SORT_FNS,
    isReady,
    hasError,
    statusLabel,
    progressPct,
  } from "./tracklist.helpers";
  import {
    listTracks,
    exportStems as exportStemsCmd,
    updateTrackMeta,
    openFolder as openFolderCmd,
    deleteTrack as deleteTrackCmd,
    retryTrack as retryTrackCmd,
  } from "./commands";
  import type { Track, PipelineEvent, ProgressMap } from "./types";
  import { PENDING_ID, EVENT_PIPELINE, SortKey } from "./constants";

  interface Props {
    tracks?: Track[];
    refresh?: (() => Promise<void>) | null;
    onPlay: (track: Track) => void;
  }

  let {
    tracks = $bindable([]),
    refresh = $bindable(null),
    onPlay,
  }: Props = $props();

  let progress: ProgressMap = $state({});

  let filterQuery = $state("");
  let sortKey: SortKey = $state(SortKey.Newest);
  let filterInput: HTMLInputElement | null = $state(null);

  function matchesFilter(track: Track): boolean {
    if (!filterQuery) return true;
    return (
      fuzzy(filterQuery, track.title) || fuzzy(filterQuery, track.artist ?? "")
    );
  }

  let displayTracks = $derived(
    tracks.filter(matchesFilter).sort(SORT_FNS[sortKey]),
  );

  let unlisten: (() => void) | undefined;

  onMount(async () => {
    refresh = refreshTracks;
    unlisten = await listen<PipelineEvent>(EVENT_PIPELINE, (event) => {
      const { track_id, stage, status, message } = event.payload;
      progress = {
        ...progress,
        [track_id]: { stage, status, message },
      };
      // Refresh from DB whenever any stage finishes or errors
      if (status === "done" || status === "error") {
        refreshTracks();
      }
    });
    await refreshTracks();
  });

  onDestroy(() => unlisten?.());

  async function refreshTracks() {
    tracks = await listTracks();
  }

  let editingId: string | null = $state(null);
  let editTitle = $state("");
  let editArtist = $state("");

  let openMenuId: string | null = $state(null);

  function startEdit(track: Track): void {
    editingId = track.id;
    editTitle = track.title;
    editArtist = track.artist ?? "";
  }

  async function commitEdit(track: Track): Promise<void> {
    if (editingId !== track.id) return;
    editingId = null;
    const trimmedTitle = editTitle.trim() || track.title;
    const trimmedArtist = editArtist.trim() || null;
    if (
      trimmedTitle === track.title &&
      trimmedArtist === (track.artist ?? null)
    )
      return;
    editError = "";
    try {
      await updateTrackMeta(track.id, trimmedTitle, trimmedArtist);
      await refreshTracks();
    } catch (e) {
      editError = String(e);
      await refreshTracks();
    }
  }

  function onEditKeydown(e: KeyboardEvent, track: Track): void {
    if (e.key === "Enter") {
      (e.target as HTMLInputElement).blur();
    }
    if (e.key === "Escape") {
      editingId = null;
    }
  }

  let editError = $state("");

  let exportingId: string | null = $state(null);
  let exportError = $state("");

  async function exportStems(track: Track): Promise<void> {
    const dest = await openDialog({
      directory: true,
      title: "Export stems to…",
    });
    if (!dest) return;
    exportingId = track.id;
    exportError = "";
    try {
      await exportStemsCmd(track.id, dest);
      await refreshTracks();
    } catch (e) {
      exportError = String(e);
    } finally {
      exportingId = null;
    }
  }

  let deleteError = $state("");
  let deletingId: string | null = $state(null);

  let retryingId: string | null = $state(null);
  let retryError = $state("");

  async function retryTrack(track: Track): Promise<void> {
    retryingId = track.id;
    retryError = "";
    try {
      await retryTrackCmd(track.id);
      await refreshTracks();
    } catch (e) {
      retryError = String(e);
    } finally {
      retryingId = null;
    }
  }

  async function deleteTrack(track: Track): Promise<void> {
    const ok = await confirm(
      `"${track.title}" and all its stems will be permanently deleted.`,
      {
        title: "Delete track?",
        kind: "warning",
        okLabel: "Delete",
        cancelLabel: "Cancel",
      },
    );
    if (!ok) return;
    deletingId = track.id;
    deleteError = "";
    try {
      await deleteTrackCmd(track.id);
      tracks = tracks.filter((t) => t.id !== track.id);
      progress = Object.fromEntries(
        Object.entries(progress).filter(([k]) => k !== String(track.id)),
      );
    } catch (e) {
      deleteError = String(e);
    } finally {
      deletingId = null;
    }
  }

  async function openFolder(path: string): Promise<void> {
    try {
      await openFolderCmd(path);
    } catch (e) {
      exportError = String(e);
    }
  }

  function isProcessing(track: Track): boolean {
    return !isReady(track) && !hasError(track, progress);
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape" && openMenuId) {
      openMenuId = null;
      return;
    }
    if (
      e.key === "s" &&
      !e.metaKey &&
      !e.ctrlKey &&
      e.target === document.body
    ) {
      e.preventDefault();
      filterInput?.focus();
    }
  }}
  onclick={(e) => {
    openMenuId = null;
    if (
      editingId &&
      !(e.target as HTMLElement | null)?.closest?.(".edit-input")
    ) {
      (document.activeElement as HTMLElement | null)?.blur?.();
    }
  }}
/>

<div class="track-list">
  <div class="toolbar">
    <div class="search-wrap">
      <svg
        class="search-icon"
        viewBox="0 0 24 24"
        width="16"
        height="16"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <circle cx="11" cy="11" r="8" />
        <line x1="21" y1="21" x2="16.65" y2="16.65" />
      </svg>
      <input
        class="filter-input"
        placeholder="Search track or artist…"
        bind:value={filterQuery}
        bind:this={filterInput}
      />
    </div>
    <div class="sort-wrap">
      <span class="sort-label">Sort:</span>
      <select class="sort-select" bind:value={sortKey}>
        <option value={SortKey.Newest}>Newest</option>
        <option value={SortKey.Oldest}>Oldest</option>
        <option value={SortKey.Title}>Title</option>
        <option value={SortKey.Artist}>Artist</option>
      </select>
    </div>
  </div>

  <header class="library-header">
    <h2>Music Library</h2>
    <p>Manage and export your separated audio stems.</p>
  </header>

  <div class="tracks-scroll">
    {#if editError}
      <p class="export-error">
        {editError}
        <button class="dismiss-error" onclick={() => (editError = "")}>×</button
        >
      </p>
    {/if}

    {#if deleteError}
      <p class="export-error">
        {deleteError}
        <button class="dismiss-error" onclick={() => (deleteError = "")}
          >×</button
        >
      </p>
    {/if}

    {#if exportError}
      <p class="export-error">
        {exportError}
        <button class="dismiss-error" onclick={() => (exportError = "")}
          >×</button
        >
      </p>
    {/if}

    {#if retryError}
      <p class="export-error">
        {retryError}
        <button class="dismiss-error" onclick={() => (retryError = "")}
          >×</button
        >
      </p>
    {/if}

    {#if tracks.length === 0}
      <p class="empty">
        No tracks yet. Add a YouTube URL or open a local file.
      </p>
    {/if}

    {#each displayTracks as track (track.id)}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div
        class="track"
        class:ready={isReady(track)}
        class:error={hasError(track, progress)}
        class:pending={track.id === PENDING_ID}
        class:playable={isReady(track)}
        role={isReady(track) ? "button" : undefined}
        tabindex={isReady(track) ? 0 : undefined}
        onclick={() => {
          if (isReady(track) && editingId !== track.id) onPlay(track);
        }}
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
          {:else}
            <span
              class="title"
              onclick={(e) => {
                e.stopPropagation();
                startEdit(track);
              }}
              onkeydown={(e) => {
                e.stopPropagation();
                e.key === "Enter" && startEdit(track);
              }}
              role="button"
              tabindex="0"
            >
              {track.title}
            </span>
          {/if}
          {#if track.id !== PENDING_ID && !isReady(track)}
            <span class="status" class:processing={isProcessing(track)}>
              {statusLabel(track, progress)}
            </span>
            {#if isProcessing(track)}
              <div class="progress-bar">
                <div
                  class="progress-fill"
                  style="width: {progressPct(track, progress)}%"
                ></div>
              </div>
            {/if}
          {/if}
        </div>
        <div class="track-artist">
          {#if track.id === PENDING_ID}
            <!-- empty cell -->
          {:else if editingId === track.id}
            <input
              class="edit-input artist-input"
              placeholder="Artist"
              bind:value={editArtist}
              onblur={() => commitEdit(track)}
              onkeydown={(e) => onEditKeydown(e, track)}
            />
          {:else}
            <span
              class="artist"
              onclick={(e) => {
                e.stopPropagation();
                startEdit(track);
              }}
              onkeydown={(e) => {
                e.stopPropagation();
                e.key === "Enter" && startEdit(track);
              }}
              role="button"
              tabindex="0"
            >
              {track.artist ?? "—"}
            </span>
          {/if}
        </div>
        <div class="track-actions">
          {#if track.id === PENDING_ID}
            <span class="spinner" aria-label="Adding track"></span>
          {:else}
            {#if isReady(track)}
              {#if track.export_path}
                <button
                  class="btn open-btn"
                  onclick={(e) => {
                    e.stopPropagation();
                    openFolder(track.export_path!);
                  }}
                  title={track.export_path!}
                  disabled={exportingId === track.id || deletingId === track.id}
                >
                  <svg
                    viewBox="0 0 24 24"
                    width="14"
                    height="14"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    aria-hidden="true"
                  >
                    <path
                      d="M4 4h5l2 3h9a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1z"
                    />
                  </svg>
                  Open
                </button>
              {/if}
              <button
                class="btn export-btn"
                onclick={(e) => {
                  e.stopPropagation();
                  exportStems(track);
                }}
                disabled={exportingId === track.id || deletingId === track.id}
              >
                <svg
                  viewBox="0 0 24 24"
                  width="14"
                  height="14"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  aria-hidden="true"
                >
                  <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                  <polyline points="7 10 12 15 17 10" />
                  <line x1="12" y1="15" x2="12" y2="3" />
                </svg>
                {exportingId === track.id ? "Exporting…" : "Export"}
              </button>
            {/if}
            {#if hasError(track, progress)}
              <button
                class="retry-btn"
                onclick={(e) => {
                  e.stopPropagation();
                  retryTrack(track);
                }}
                disabled={retryingId === track.id}
              >
                {retryingId === track.id ? "Retrying…" : "↺ Retry"}
              </button>
            {/if}
            <div class="track-menu">
              <button
                class="menu-trigger"
                onclick={(e) => {
                  e.stopPropagation();
                  openMenuId = openMenuId === track.id ? null : track.id;
                }}
                disabled={exportingId === track.id ||
                  deletingId === track.id ||
                  retryingId === track.id}
                aria-haspopup="menu"
                aria-expanded={openMenuId === track.id}
                aria-label="Track actions"
              >
                <svg
                  viewBox="0 0 24 24"
                  width="16"
                  height="16"
                  fill="currentColor"
                  aria-hidden="true"
                >
                  <circle cx="12" cy="5" r="1.6" />
                  <circle cx="12" cy="12" r="1.6" />
                  <circle cx="12" cy="19" r="1.6" />
                </svg>
              </button>
              {#if openMenuId === track.id}
                <div class="menu-dropdown" role="menu">
                  <button
                    class="menu-item destructive"
                    role="menuitem"
                    onclick={(e) => {
                      e.stopPropagation();
                      openMenuId = null;
                      deleteTrack(track);
                    }}
                  >
                    <svg
                      viewBox="0 0 24 24"
                      width="14"
                      height="14"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="2"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      aria-hidden="true"
                    >
                      <polyline points="3 6 5 6 21 6" />
                      <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6" />
                      <path d="M10 11v6" />
                      <path d="M14 11v6" />
                      <path d="M9 6V4a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2v2" />
                    </svg>
                    Delete track
                  </button>
                </div>
              {/if}
            </div>
          {/if}
        </div>
      </div>
    {/each}
  </div>
</div>

<style>
  .track-list {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }

  .toolbar {
    display: flex;
    gap: 12px;
    align-items: center;
    margin-inline: -24px;
    padding: 0 24px 12px;
    margin-bottom: 12px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .search-wrap {
    position: relative;
    flex: 1;
    max-width: 480px;
  }

  .search-icon {
    position: absolute;
    left: 16px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--fg);
    pointer-events: none;
  }

  .filter-input {
    width: 100%;
    background: var(--bg-panel);
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--fg);
    padding: 10px 18px 10px 40px;
    font-size: 13px;
    outline: none;
  }

  .filter-input:focus {
    border-color: var(--color-ready);
  }

  .filter-input::placeholder {
    color: var(--fg-muted);
  }

  .sort-wrap {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 14px;
    background: var(--bg-panel);
    border: 1px solid var(--border);
    border-radius: 8px;
    flex-shrink: 0;
    margin-left: auto;
  }

  .sort-label {
    color: var(--fg-muted);
    font-size: 13px;
  }

  .sort-select {
    appearance: none;
    -webkit-appearance: none;
    background: transparent;
    border: none;
    color: var(--color-ready);
    font-family: inherit;
    font-size: 13px;
    font-weight: 500;
    outline: none;
    cursor: pointer;
    padding-right: 16px;
    background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10' viewBox='0 0 24 24' fill='none' stroke='%234caf72' stroke-width='2.5' stroke-linecap='round' stroke-linejoin='round'><polyline points='6 9 12 15 18 9'/></svg>");
    background-repeat: no-repeat;
    background-position: right 0 center;
  }

  .sort-select option {
    background: var(--bg-panel);
    color: var(--fg);
  }

  .library-header {
    margin: 0 0 12px;
    flex-shrink: 0;
  }

  .library-header h2 {
    margin: 0 0 4px;
    font-size: 22px;
    font-weight: 600;
    color: var(--fg);
  }

  .library-header p {
    margin: 0;
    font-size: 13px;
    color: var(--fg-muted);
  }

  .tracks-scroll {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
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
    display: grid;
    grid-template-columns: minmax(0, 400px) minmax(0, 240px) 1fr 200px;
    align-items: center;
    padding: 7px 12px;
    border-left: 3px solid transparent;
    border-radius: 6px;
    background: var(--bg-track);
    column-gap: 16px;
  }

  .track.ready {
    border-left-color: var(--color-ready);
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

  .track-artist {
    min-width: 0;
  }

  .title {
    align-self: flex-start;
    width: fit-content;
    min-width: 40px;
    max-width: 100%;
    font-size: 14px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: text;
  }

  .artist {
    display: block;
    width: fit-content;
    min-width: 40px;
    max-width: 100%;
    font-size: 13px;
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
    justify-content: flex-end;
    gap: 10px;
    justify-self: end;
    grid-column: 4;
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

  .track-menu {
    position: relative;
    display: inline-flex;
  }

  .menu-trigger {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 4px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
    cursor: pointer;
    line-height: 0;
  }

  .menu-trigger:hover:not(:disabled) {
    color: var(--fg);
    background: var(--bg-button-hover);
  }

  .menu-trigger:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .menu-dropdown {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    z-index: 10;
    min-width: 160px;
    padding: 4px;
    background: var(--bg-panel);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  }

  .menu-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 10px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--fg);
    font-family: inherit;
    font-size: 13px;
    text-align: left;
    cursor: pointer;
    white-space: nowrap;
  }

  .menu-item:hover {
    background: var(--bg-button-hover);
  }

  .menu-item.destructive {
    color: var(--color-error);
  }

  .menu-item.destructive:hover {
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
    to {
      transform: rotate(360deg);
    }
  }
</style>
