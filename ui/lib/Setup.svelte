<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { onDestroy } from "svelte";
  import { downloadDemucs } from "./commands";
  import type { SetupProgress } from "./types";

  interface Props {
    onReady: () => void;
  }

  let { onReady }: Props = $props();

  let downloading = $state(false);
  let error: string | null = $state(null);
  let progress: SetupProgress | null = $state(null);

  let unlisten: (() => void) | undefined;
  onDestroy(() => unlisten?.());

  async function startDownload() {
    if (downloading) return;
    unlisten?.();
    downloading = true;
    error = null;
    progress = null;

    unlisten = await listen<SetupProgress>("setup:progress", (e) => {
      progress = e.payload;
    });

    try {
      await downloadDemucs();
      onReady();
    } catch (e) {
      error = String(e);
      downloading = false;
    }
  }
</script>

<div class="card">
  <h2>One-time setup</h2>
  <p class="description">
    Before you can split your first track, Wavesplit needs to download the audio
    separation engine. This only happens once. After that, everything works
    offline.
  </p>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if downloading && progress}
    <div class="progress-wrap">
      <div class="progress-bar">
        <div
          class="progress-fill"
          style="width: {progress.percent ?? 0}%"
        ></div>
      </div>
      <p class="progress-label">
        {#if progress.total_mb}
          {progress.downloaded_mb.toFixed(0)} / {progress.total_mb.toFixed(0)} MB
          ({progress.percent}%)
        {:else}
          {progress.downloaded_mb.toFixed(0)} MB downloaded…
        {/if}
      </p>
    </div>
  {:else if downloading}
    <p class="progress-label">Connecting…</p>
  {:else}
    <button onclick={startDownload}>Download</button>
  {/if}

  <p class="note">
    Make sure you're on Wi-Fi. The first time you split a track, an additional
    ~80 MB will be fetched automatically.
  </p>
</div>

<style>
  .card {
    background: var(--bg-panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 32px;
    max-width: 420px;
    width: 100%;
  }

  h2 {
    margin: 0 0 12px;
    font-size: 18px;
    font-weight: 600;
  }

  .description {
    margin: 0 0 20px;
    color: var(--fg-muted);
    line-height: 1.5;
  }

  button {
    width: 100%;
    padding: 10px;
    background: var(--accent);
    color: #fff;
    border: none;
    border-radius: 6px;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
  }

  button:hover {
    opacity: 0.9;
  }

  .progress-wrap {
    margin-bottom: 16px;
  }

  .progress-bar {
    height: 6px;
    background: var(--bg-input);
    border-radius: 3px;
    overflow: hidden;
    margin-bottom: 6px;
  }

  .progress-fill {
    height: 100%;
    background: var(--accent);
    transition: width 0.2s ease;
  }

  .progress-label {
    margin: 0;
    font-size: 12px;
    color: var(--fg-muted);
    text-align: center;
  }

  .error {
    margin: 0 0 16px;
    color: var(--color-error);
    font-size: 13px;
  }

  .note {
    margin: 16px 0 0;
    font-size: 12px;
    color: var(--fg-muted);
    line-height: 1.4;
  }
</style>
