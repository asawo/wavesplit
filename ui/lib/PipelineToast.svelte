<script>
  import { stageLabel, nextStage } from "./tracklist.helpers.js";

  let { title, stage, status, message, canCancel, onCancel, onDismiss } =
    $props();

  function toastLabel(stage, status, message) {
    if (status === "error") return message ? `Error: ${message}` : "Failed";
    if (status === "done" && stage === "analysis") return "Done";
    if (status === "done") return stageLabel(nextStage(stage));
    return stageLabel(stage);
  }

  let isDone = $derived(stage === "analysis" && status === "done");
  let isError = $derived(status === "error");
  let isInProgress = $derived(!isDone && !isError);
</script>

<div class="pipeline-toast fade-in" class:done={isDone} class:error={isError}>
  <div class="toast-body">
    {#if isInProgress}
      <span class="spinner" aria-label="Processing"></span>
    {:else if isDone}
      <span class="icon done-icon">✓</span>
    {:else}
      <span class="icon error-icon">✕</span>
    {/if}
    <div class="toast-text">
      <span class="toast-title">{title}</span>
      <span class="toast-status">{toastLabel(stage, status, message)}</span>
    </div>
  </div>
  {#if isInProgress && canCancel}
    <button class="cancel-btn" onclick={onCancel} aria-label="Cancel">
      <svg
        width="13"
        height="13"
        viewBox="0 0 13 13"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
      >
        <rect
          x="1"
          y="3"
          width="11"
          height="1.2"
          rx="0.6"
          fill="currentColor"
        />
        <path
          d="M4.5 3V2a.5.5 0 0 1 .5-.5h3a.5.5 0 0 1 .5.5v1"
          stroke="currentColor"
          stroke-width="1.2"
          stroke-linecap="round"
        />
        <rect
          x="3"
          y="4.5"
          width="1.2"
          height="6"
          rx="0.6"
          fill="currentColor"
        />
        <rect
          x="5.9"
          y="4.5"
          width="1.2"
          height="6"
          rx="0.6"
          fill="currentColor"
        />
        <rect
          x="8.8"
          y="4.5"
          width="1.2"
          height="6"
          rx="0.6"
          fill="currentColor"
        />
        <rect
          x="2"
          y="4"
          width="9"
          height="7"
          rx="1"
          stroke="currentColor"
          stroke-width="1.2"
          fill="none"
        />
      </svg>
    </button>
  {:else if isDone || isError}
    <button class="dismiss-btn" onclick={onDismiss} aria-label="Dismiss"
      >✕</button
    >
  {/if}
</div>

<style>
  .pipeline-toast {
    position: fixed;
    bottom: 10px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 50;
    background: var(--bg-panel);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 10px;
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 260px;
    max-width: 440px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
  }

  .pipeline-toast.done {
    border-color: var(--color-ready);
  }

  .pipeline-toast.error {
    border-color: var(--color-error);
  }

  .toast-body {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
  }

  .toast-text {
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .toast-title {
    font-size: 12px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .toast-status {
    font-size: 11px;
    color: var(--fg-muted);
  }

  .pipeline-toast.done .toast-status {
    color: var(--color-ready);
  }

  .pipeline-toast.error .toast-status {
    color: var(--color-error);
  }

  .icon {
    font-size: 14px;
    flex-shrink: 0;
  }

  .done-icon {
    color: var(--color-ready);
  }

  .error-icon {
    color: var(--color-error);
  }

  .spinner {
    display: inline-block;
    width: 12px;
    height: 12px;
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

  .cancel-btn {
    padding: 0 4px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-error);
    font-size: 14px;
    cursor: pointer;
    flex-shrink: 0;
    line-height: 1;
    opacity: 0.7;
  }

  .cancel-btn:hover {
    opacity: 1;
  }

  .dismiss-btn {
    padding: 3px 7px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
    font-size: 13px;
    cursor: pointer;
    flex-shrink: 0;
    line-height: 1;
  }

  .dismiss-btn:hover {
    color: var(--fg);
  }
</style>
