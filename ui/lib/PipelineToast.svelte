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
    <button class="cancel-btn" onclick={onCancel}>Cancel</button>
  {:else if isDone || isError}
    <button class="dismiss-btn" onclick={onDismiss} aria-label="Dismiss"
      >✕</button
    >
  {/if}
</div>

<style>
  .pipeline-toast {
    position: fixed;
    bottom: 20px;
    right: 20px;
    z-index: 50;
    background: var(--bg-panel);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 10px 12px;
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 220px;
    max-width: 320px;
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
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .toast-title {
    font-size: 13px;
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

  .cancel-btn {
    padding: 3px 10px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
    font-size: 12px;
    cursor: pointer;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .cancel-btn:hover {
    border-color: var(--color-error);
    color: var(--color-error);
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
