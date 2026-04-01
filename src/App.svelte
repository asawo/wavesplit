<script>
  import { invoke } from '@tauri-apps/api/core'
  import { onMount } from 'svelte'
  import AddTrack from './lib/AddTrack.svelte'
  import TrackList from './lib/TrackList.svelte'
  import Setup from './lib/Setup.svelte'

  let tracks = $state([])
  let refreshTracks = $state(null)
  let ready = $state(true)  // optimistic: assume available, overlay shows if not

  onMount(async () => {
    ready = await invoke('check_demucs')
  })

  function handleAdded(_id) {
    refreshTracks?.()
  }
</script>

<div class="app fade-in">
  <header>
    <h1>Wavesplit</h1>
  </header>

  <main>
    <section class="add-section">
      <p class="section-label">Add track</p>
      <AddTrack onAdded={handleAdded} />
    </section>

    <section class="list-section">
      <p class="section-label">Library</p>
      <TrackList bind:tracks bind:refresh={refreshTracks} />
    </section>
  </main>
</div>

{#if !ready}
  <div class="overlay fade-in">
    <Setup onReady={() => ready = true} />
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
    from { opacity: 0; }
    to   { opacity: 1; }
  }

  :global(.fade-in) {
    animation: fade-in 0.15s ease-out both;
  }

  :global(body) {
    margin: 0;
    background: var(--bg);
    color: var(--fg);
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
    font-size: 14px;
    -webkit-font-smoothing: antialiased;
  }

  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }

  header {
    padding: 14px 20px 12px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  h1 {
    margin: 0;
    font-size: 24px;
    font-family: 'Oleo Script Swash Caps', cursive;
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
    overflow-y: auto;
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
