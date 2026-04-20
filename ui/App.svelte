<script>
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";
  import AddTrack from "./lib/AddTrack.svelte";
  import TrackList from "./lib/TrackList.svelte";
  import Playback from "./lib/Playback.svelte";
  import Setup from "./lib/Setup.svelte";
  import PipelineToast from "./lib/PipelineToast.svelte";

  let tracks = $state([]);
  let refreshTracks = $state(null);
  let ready = $state(true); // optimistic: assume available, overlay shows if not

  let screen = $state("library"); // 'library' | 'playback'
  let selectedTrack = $state(null);

  let toastTrack = $state(null);
  let toastDismissTimer = null;
  let unlistenPipeline;

  onDestroy(() => {
    unlistenPipeline?.();
    clearTimeout(toastDismissTimer);
  });

  onMount(async () => {
    try {
      ready = await invoke("check_demucs");
    } catch {
      ready = false;
    }
    unlistenPipeline = await listen("pipeline", ({ payload }) => {
      const { track_id, stage, status, message } = payload;
      if (!toastTrack || toastTrack.id !== track_id) return;
      toastTrack.stage = stage;
      toastTrack.status = status;
      toastTrack.message = message ?? "";
      if (stage === "analysis" && status === "done") {
        clearTimeout(toastDismissTimer);
        toastDismissTimer = setTimeout(() => {
          toastTrack = null;
        }, 2000);
      }
    });
  });

  const PENDING_ID = "__pending__";

  function handleStarted(title) {
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
      },
      ...tracks,
    ];
    toastTrack = {
      id: null,
      title,
      stage: "download",
      status: "pending",
      message: "",
    };
  }

  async function handleAdded(id) {
    await refreshTracks?.();
    if (toastTrack) {
      if (id) {
        toastTrack.id = id;
        const track = tracks.find((t) => t.id === id);
        if (track) toastTrack.title = track.title;
      } else {
        toastTrack = null;
      }
    }
  }

  async function handleCancelToast() {
    const id = toastTrack?.id;
    toastTrack = null;
    if (id) {
      await invoke("delete_track", { id });
      refreshTracks?.();
    }
  }

  function dismissToast() {
    clearTimeout(toastDismissTimer);
    toastTrack = null;
  }

  function openPlayback(track) {
    selectedTrack = track;
    screen = "playback";
  }

  function closePlayback() {
    screen = "library";
    // keep selectedTrack alive so playhead position is preserved on return
  }
</script>

<div class="app fade-in">
  <div class="screens-inner" class:show-playback={screen === "playback"}>
    <!-- Library screen -->
    <div class="screen">
      <header>
        <h1>Wavesplit</h1>
      </header>
      <main>
        <section class="add-section">
          <p class="section-label">Add track</p>
          <AddTrack onAdded={handleAdded} onStarted={handleStarted} />
        </section>
        <section class="list-section">
          <p class="section-label">Library</p>
          <TrackList
            bind:tracks
            bind:refresh={refreshTracks}
            onPlay={openPlayback}
          />
        </section>
      </main>
    </div>

    <!-- Playback screen -->
    <div class="screen">
      {#if selectedTrack}
        <Playback
          track={selectedTrack}
          active={screen === "playback"}
          onBack={closePlayback}
        />
      {/if}
    </div>
  </div>
</div>

{#if toastTrack}
  <PipelineToast
    title={toastTrack.title}
    stage={toastTrack.stage}
    status={toastTrack.status}
    message={toastTrack.message}
    canCancel={!!toastTrack.id}
    onCancel={handleCancelToast}
    onDismiss={dismissToast}
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
    --bg: #1a1a1a;
    --bg-panel: #202020;
    --bg-input: #2a2a2a;
    --bg-button: #2a2a2a;
    --bg-button-hover: #363636;
    --bg-track: #252525;
    --bg-track-ready: #1e2a1e;
    --bg-track-error: #2a1e1e;
    --fg: #e8e8e8;
    --fg-muted: #888;
    --border: #3a3a3a;
    --accent: #4a9eff;
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

  header {
    padding: 14px 20px 12px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  h1 {
    margin: 0;
    font-size: 24px;
    font-family: "Oleo Script Swash Caps", cursive;
    font-weight: 400;
    color: var(--color-ready);
  }

  main {
    display: flex;
    flex-direction: column;
    flex: 1;
    overflow: hidden;
    padding: 16px 20px;
    gap: 20px;
  }

  .section-label {
    margin: 0 0 8px;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--fg-muted);
  }

  .add-section {
    flex-shrink: 0;
    background: var(--bg-panel);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 12px 14px;
  }

  .list-section {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
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
