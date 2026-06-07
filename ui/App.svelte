<script lang="ts">
  import { onMount } from "svelte";
  import TrackList from "./lib/TrackList.svelte";
  import Playback from "./lib/Playback.svelte";
  import Setup from "./lib/Setup.svelte";
  import Sidebar from "./lib/Sidebar.svelte";
  import AddTrackModal from "./lib/AddTrackModal.svelte";
  import { checkDemucs } from "./lib/commands";
  import type { Track } from "./lib/types";
  import { PENDING_ID, Screen } from "./lib/constants";

  let tracks: Track[] = $state([]);
  let refreshTracks: (() => Promise<void>) | null = $state(null);
  let ready = $state(true); // optimistic: assume available, overlay shows if not

  let screen: Screen = $state(Screen.Library);
  let selectedTrack: Track | null = $state(null);

  onMount(async () => {
    try {
      ready = await checkDemucs();
    } catch {
      ready = false;
    }
  });

  function handleStarted(title: string): void {
    tracks = [
      {
        id: PENDING_ID,
        title,
        artist: null,
        sort_order: Date.now(),
        status_download: "pending",
        status_stems: "pending",
        status_analysis: "pending",
        error_message: null,
        export_path: null,
        duration_ms: null,
        source_type: "local",
        source_url: null,
        source_path: null,
        created_at: "",
      },
      ...tracks,
    ];
  }

  async function handleAdded(_id: string | null): Promise<void> {
    await refreshTracks?.();
  }

  function openPlayback(track: Track): void {
    selectedTrack = track;
    screen = Screen.Playback;
  }

  function closePlayback(): void {
    screen = Screen.Library;
    // keep selectedTrack alive so playhead position is preserved on return
  }

  async function handleExportDone(): Promise<void> {
    await refreshTracks?.();
  }

  let showAddModal = $state(false);

  function openAddModal(): void {
    showAddModal = true;
  }

  function closeAddModal(): void {
    showAddModal = false;
  }
</script>

<div class="app fade-in">
  <Sidebar
    activeId="library"
    onSelect={(id) => {
      if (id === "library") closePlayback();
    }}
    onImport={openAddModal}
  />
  <div class="content">
    <div class="screens-inner" class:show-playback={screen === Screen.Playback}>
      <!-- Library screen -->
      <div class="screen">
        <main>
          <TrackList
            bind:tracks
            bind:refresh={refreshTracks}
            onPlay={openPlayback}
          />
        </main>
      </div>

      <!-- Playback screen -->
      <div class="screen">
        {#if selectedTrack}
          <Playback
            track={selectedTrack}
            active={screen === Screen.Playback}
            onBack={closePlayback}
            onExportDone={handleExportDone}
          />
        {/if}
      </div>
    </div>
  </div>
</div>

{#if showAddModal}
  <AddTrackModal
    onClose={closeAddModal}
    onStarted={handleStarted}
    onAdded={handleAdded}
  />
{/if}

{#if !ready}
  <div class="overlay fade-in">
    <Setup onReady={() => (ready = true)} />
  </div>
{/if}

<style>
  :global(*) {
    box-sizing: border-box;
  }

  :global(:root) {
    --bg: #0a0a0a;
    --bg-sidebar: #121212;
    --bg-panel: #161616;
    --bg-input: #1f1f1f;
    --bg-button: #1f1f1f;
    --bg-button-hover: #2a2a2a;
    --bg-track: #161616;
    --bg-track-ready: #142014;
    --bg-track-error: #201414;
    --fg: #e8e8e8;
    --fg-muted: #888;
    --border: #2a2a2a;
    --accent: #4a9eff;
    --accent-warm: #e87e3a;
    --color-error: #ff6b6b;
    --color-processing: #f0a030;
    --color-ready: #4caf72;
  }

  @keyframes fade-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  :global(.fade-in) {
    animation: fade-in 0.15s ease-out both;
  }

  /* Base button: default radius, type, layout. Apply with class="btn …" */
  :global(.btn) {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border: 1px solid transparent;
    border-radius: 6px;
    background: transparent;
    color: var(--fg);
    font-family: inherit;
    font-size: 12px;
    font-weight: 400;
    cursor: pointer;
    white-space: nowrap;
  }

  :global(.btn:disabled) {
    opacity: 0.5;
    cursor: default;
  }

  :global(.btn svg) {
    flex-shrink: 0;
  }

  /* Variants — only set what differs from .btn */

  :global(.open-btn) {
    border-color: var(--border);
  }

  :global(.open-btn:hover:not(:disabled)) {
    border-color: var(--fg-muted);
    background: var(--bg-button-hover);
  }

  :global(.export-btn) {
    border-color: var(--color-ready);
    background: var(--color-ready);
    color: #0a0a0a;
    font-weight: 500;
  }

  :global(.export-btn:hover:not(:disabled)) {
    background: #5cc283;
    border-color: #5cc283;
  }

  :global(body) {
    margin: 0;
    background: var(--bg);
    color: var(--fg);
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    font-size: 14px;
    -webkit-font-smoothing: antialiased;
  }

  .app {
    height: 100vh;
    overflow: hidden;
    display: flex;
  }

  .content {
    flex: 1;
    min-width: 0;
    height: 100%;
    overflow: hidden;
  }

  .screens-inner {
    display: flex;
    width: 200%;
    height: 100%;
    transition: transform 0.2s ease-out;
    will-change: transform;
  }

  .screens-inner.show-playback {
    transform: translateX(-50%);
  }

  .screen {
    width: 50%;
    height: 100%;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  main {
    display: flex;
    flex-direction: column;
    flex: 1;
    overflow: hidden;
  }

  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.75);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    backdrop-filter: blur(4px);
  }
</style>
