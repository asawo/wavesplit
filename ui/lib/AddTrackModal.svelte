<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { addTrackYoutube } from "./commands";
  import { pickAndImportLocal, importLocalPath } from "./importTrack";

  interface Props {
    onClose: () => void;
    onStarted: (title: string) => void;
    onAdded: (id: string | null) => Promise<void>;
  }

  let { onClose, onStarted, onAdded }: Props = $props();

  let url = $state("");
  let loading = $state(false);
  let error = $state("");
  let isDragging = $state(false);

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

  async function submitYoutube(): Promise<void> {
    if (!url.trim() || !isValidYoutubeUrl(url)) return;
    const pendingUrl = normalizeUrl(url);
    url = "";
    onStarted(pendingUrl);
    onClose();
    // Modal is closed; the pipeline toast surfaces progress and errors.
    try {
      const result = await addTrackYoutube(pendingUrl);
      await onAdded(result.id);
    } catch {
      await onAdded(null);
    }
  }

  async function chooseFile(): Promise<void> {
    loading = true;
    error = "";
    try {
      let started = false;
      await pickAndImportLocal({
        onStarted: (title) => {
          started = true;
          onStarted(title);
        },
        onAdded,
        onError: (msg) => (error = msg),
      });
      if (started && !error) onClose();
    } finally {
      loading = false;
    }
  }

  async function importDroppedFile(path: string): Promise<void> {
    loading = true;
    error = "";
    try {
      let started = false;
      await importLocalPath(path, {
        onStarted: (title) => {
          started = true;
          onStarted(title);
        },
        onAdded,
        onError: (msg) => (error = msg),
      });
      if (started && !error) onClose();
    } finally {
      loading = false;
    }
  }

  let unlistenDragDrop: (() => void) | undefined;

  onMount(async () => {
    unlistenDragDrop = await getCurrentWebview().onDragDropEvent((event) => {
      if (loading) return;
      const p = event.payload;
      if (p.type === "enter" || p.type === "over") {
        isDragging = true;
      } else if (p.type === "leave") {
        isDragging = false;
      } else if (p.type === "drop") {
        isDragging = false;
        const path = p.paths[0];
        if (path) importDroppedFile(path);
      }
    });
  });

  onDestroy(() => unlistenDragDrop?.());

  function onBackdropClick(): void {
    if (!loading) onClose();
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === "Escape" && !loading) onClose();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onBackdropClick}>
  <div
    class="modal"
    role="dialog"
    aria-modal="true"
    aria-labelledby="add-track-title"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
  >
    <header>
      <h2 id="add-track-title">Add Track</h2>
      <button
        class="close-btn"
        type="button"
        onclick={onClose}
        disabled={loading}
        aria-label="Close"
      >
        <svg
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
          <line x1="6" y1="6" x2="18" y2="18" />
          <line x1="18" y1="6" x2="6" y2="18" />
        </svg>
      </button>
    </header>

    <section class="field">
      <label for="yt-url">Import YouTube URL</label>
      <div class="url-panel">
        <span class="link-icon" aria-hidden="true">
          <svg
            viewBox="0 0 24 24"
            width="16"
            height="16"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path
              d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"
            />
            <path
              d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"
            />
          </svg>
        </span>
        <input
          id="yt-url"
          type="text"
          placeholder="Paste YouTube link here…"
          bind:value={url}
          disabled={loading}
          onkeydown={(e) => e.key === "Enter" && submitYoutube()}
        />
        <button
          class="btn export-btn add-btn"
          type="button"
          onclick={submitYoutube}
          disabled={loading || !isValidYoutubeUrl(url)}
        >
          Add
        </button>
      </div>
      {#if urlError}
        <p class="hint error">{urlError}</p>
      {/if}
    </section>

    <div class="divider"><span>or</span></div>

    <section class="field">
      <span class="label">Import Local File</span>
      <button
        class="drop-zone"
        class:dragging={isDragging}
        type="button"
        onclick={chooseFile}
        disabled={loading}
      >
        <span class="drop-icon" aria-hidden="true">
          <svg
            viewBox="0 0 24 24"
            width="22"
            height="22"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path
              d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"
            />
            <polyline points="14 2 14 8 20 8" />
            <line x1="12" y1="18" x2="12" y2="12" />
            <polyline points="9 15 12 12 15 15" />
          </svg>
        </span>
        <span class="drop-title">Choose audio file</span>
        <span class="drop-hint">Supported: MP3, WAV, FLAC, M4A</span>
      </button>
    </section>

    {#if error}
      <p class="error">{error}</p>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
    backdrop-filter: blur(4px);
  }

  .modal {
    width: min(480px, calc(100vw - 32px));
    background: var(--bg-panel);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 18px;
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.5);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: var(--fg);
  }

  .close-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 4px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
    cursor: pointer;
  }

  .close-btn:hover:not(:disabled) {
    color: var(--fg);
    background: var(--bg-button-hover);
  }

  .close-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  label,
  .label {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--fg-muted);
  }

  .url-panel {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 6px 6px 14px;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 999px;
  }

  .url-panel:focus-within {
    border-color: var(--color-ready);
  }

  .link-icon {
    display: inline-flex;
    color: var(--fg-muted);
    flex-shrink: 0;
  }

  .url-panel input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    padding: 6px 0;
    color: var(--fg);
    font-size: 13px;
    outline: none;
  }

  .url-panel input::placeholder {
    color: var(--fg-muted);
  }

  .add-btn {
    border-radius: 999px;
    padding: 6px 18px;
  }

  .drop-zone {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 28px 16px;
    border: 1.5px dashed var(--border);
    border-radius: 12px;
    background: transparent;
    color: var(--fg);
    font-family: inherit;
    cursor: pointer;
    transition:
      border-color 0.15s,
      background 0.15s;
  }

  .drop-zone:hover:not(:disabled) {
    border-color: var(--fg-muted);
    background: rgba(255, 255, 255, 0.02);
  }

  .drop-zone:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .drop-zone.dragging {
    border-color: var(--color-ready);
    background: rgba(76, 175, 114, 0.06);
  }

  .drop-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 44px;
    height: 44px;
    margin-bottom: 4px;
    border-radius: 10px;
    color: var(--color-ready);
    background: rgba(76, 175, 114, 0.08);
  }

  .drop-title {
    font-size: 14px;
    font-weight: 600;
  }

  .drop-hint {
    font-size: 11px;
    color: var(--fg-muted);
  }

  .divider {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--fg-muted);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    font-weight: 600;
  }

  .divider::before,
  .divider::after {
    content: "";
    flex: 1;
    height: 1px;
    background: var(--border);
  }

  .hint {
    margin: 0;
    font-size: 11px;
    color: var(--fg-muted);
  }

  .error {
    margin: 0;
    font-size: 12px;
    color: var(--color-error);
  }
</style>
