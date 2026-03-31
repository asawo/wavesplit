<script>
  import AddTrack from './lib/AddTrack.svelte'
  import TrackList from './lib/TrackList.svelte'

  let tracks = $state([])
  let refreshTracks = $state(null)

  function handleAdded(_id) {
    refreshTracks?.()
  }
</script>

<div class="app">
  <header>
    <h1>wavesplit</h1>
  </header>

  <main>
    <section class="add-section">
      <AddTrack onAdded={handleAdded} />
    </section>

    <section class="list-section">
      <TrackList bind:tracks bind:refresh={refreshTracks} />
    </section>
  </main>
</div>

<style>
  :global(*) {
    box-sizing: border-box;
  }

  :global(:root) {
    --bg: #1a1a1a;
    --bg-input: #2a2a2a;
    --bg-button: #2a2a2a;
    --bg-button-hover: #363636;
    --bg-track: #222;
    --bg-track-ready: #1e2a1e;
    --bg-track-error: #2a1e1e;
    --fg: #e8e8e8;
    --fg-muted: #888;
    --border: #3a3a3a;
    --accent: #4a9eff;
    --color-error: #ff6b6b;
    --color-processing: #f0a030;
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
    font-size: 16px;
    font-weight: 600;
    letter-spacing: 0.05em;
    color: var(--accent);
  }

  main {
    display: flex;
    flex-direction: column;
    flex: 1;
    overflow: hidden;
    padding: 16px 20px;
    gap: 16px;
  }

  .add-section {
    flex-shrink: 0;
  }

  .list-section {
    flex: 1;
    overflow-y: auto;
  }
</style>
