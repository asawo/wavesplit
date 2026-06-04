<script lang="ts">
  import { addTrackYoutube } from "./commands";
  import { pickAndImportLocal } from "./importTrack";

  interface Props {
    onAdded: (id: string | null) => Promise<void>;
    onStarted: (title: string) => void;
  }

  let { onAdded, onStarted }: Props = $props();

  let url = $state("");
  let loading = $state(false);
  let error = $state("");

  const YOUTUBE_PATTERN =
    "https://(www\\.youtube\\.com|youtu\\.be|music\\.youtube\\.com)/.+";

  function normalizeUrl(value: string): string {
    const trimmed = value.trim();
    if (trimmed && !/^https?:\/\//i.test(trimmed)) return "https://" + trimmed;
    return trimmed;
  }

  function isValidYoutubeUrl(value: string): boolean {
    return new RegExp("^" + YOUTUBE_PATTERN + "$").test(normalizeUrl(value));
  }

  let urlError = $derived(
    url.trim() && !isValidYoutubeUrl(url)
      ? "Must be a YouTube URL (youtube.com, youtu.be, music.youtube.com)"
      : "",
  );

  async function addYoutube() {
    if (!url.trim()) return;
    loading = true;
    error = "";
    const pendingUrl = normalizeUrl(url);
    url = "";
    onStarted(pendingUrl);
    try {
      const result = await addTrackYoutube(pendingUrl);
      if (result.duplicate) error = "This track is already in your library";
      onAdded(result.id);
    } catch (e) {
      error = String(e);
      onAdded(null);
    } finally {
      loading = false;
    }
  }

  async function addLocal() {
    loading = true;
    error = "";
    try {
      await pickAndImportLocal({
        onStarted,
        onAdded,
        onError: (msg) => (error = msg),
      });
    } finally {
      loading = false;
    }
  }
</script>

<div class="add-track">
  <div class="row">
    <input
      type="text"
      placeholder="YouTube URL"
      bind:value={url}
      disabled={loading}
      onkeydown={(e) => e.key === "Enter" && addYoutube()}
    />
    <button onclick={addYoutube} disabled={loading || !isValidYoutubeUrl(url)}
      >Add</button
    >
    <span class="divider">or</span>
    <button onclick={addLocal} disabled={loading}>Open file…</button>
  </div>
  {#if urlError}
    <p class="error">{urlError}</p>
  {:else if error}
    <p class="error">{error}</p>
  {/if}
</div>

<style>
  .add-track {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  input {
    flex: 1;
    padding: 6px 10px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg-input);
    color: var(--fg);
    font-size: 13px;
  }

  input:focus {
    outline: none;
    border-color: var(--accent);
  }

  button {
    padding: 6px 14px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg-button);
    color: var(--fg);
    font-size: 13px;
    cursor: pointer;
    white-space: nowrap;
  }

  button:hover:not(:disabled) {
    background: var(--bg-button-hover);
  }

  button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .divider {
    font-size: 12px;
    color: var(--fg-muted);
  }

  .error {
    font-size: 12px;
    color: var(--color-error);
    margin: 0;
  }
</style>
