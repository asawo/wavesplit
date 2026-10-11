<script lang="ts">
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { errors, dismissError } from "./errorReport.svelte";
  import {
    formatReport,
    issueUrl,
    type ReportEnv,
  } from "./errorReport.helpers";

  let env: ReportEnv = $state({ version: "", userAgent: navigator.userAgent });
  let copied = $state(false);

  let current = $derived(errors.queue[0]);
  let remaining = $derived(errors.queue.length - 1);

  onMount(async () => {
    try {
      env.version = await getVersion();
    } catch {
      // leave as "unknown" in the report
    }
  });

  function close(): void {
    copied = false;
    dismissError();
  }

  async function copyDetails(): Promise<void> {
    if (!current) return;
    try {
      await navigator.clipboard.writeText(formatReport(current, env));
      copied = true;
    } catch {
      copied = false;
    }
  }

  async function reportIssue(): Promise<void> {
    if (!current) return;
    // Also copy, so the full text is at hand if the URL had to be truncated
    await copyDetails();
    try {
      await openUrl(issueUrl(current, env));
    } catch {
      // Opening the browser failed; the details are still on the clipboard
    }
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (current && e.key === "Escape") {
      e.stopPropagation();
      close();
    }
  }}
/>

{#if current}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="backdrop fade-in" onclick={close}>
    <div
      class="modal"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="error-modal-title"
      aria-describedby="error-modal-message"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
    >
      <header>
        <h2 id="error-modal-title">
          <span class="material-symbols-rounded icon" aria-hidden="true"
            >error</span
          >
          {current.title}
        </h2>
        <button
          class="close-btn"
          type="button"
          onclick={close}
          aria-label="Close"
        >
          <span class="material-symbols-rounded" aria-hidden="true">close</span>
        </button>
      </header>

      <pre id="error-modal-message" class="message">{current.message}</pre>

      <dl class="details">
        <dt>Version</dt>
        <dd>{env.version || "unknown"}</dd>
        <dt>Time</dt>
        <dd>{current.time}</dd>
        {#each Object.entries(current.context ?? {}) as [k, v] (k)}
          <dt>{k}</dt>
          <dd>{v}</dd>
        {/each}
      </dl>

      <p class="hint">
        Please report this so it can be fixed. Check the details before
        submitting, since they can include file paths from your computer.
      </p>

      <footer>
        {#if remaining > 0}
          <span class="remaining">+{remaining} more</span>
        {/if}
        <button class="btn open-btn" type="button" onclick={copyDetails}>
          {copied ? "Copied" : "Copy details"}
        </button>
        <button class="btn open-btn" type="button" onclick={reportIssue}>
          Report on GitHub
        </button>
        <button class="btn export-btn" type="button" onclick={close}>
          {remaining > 0 ? "Next" : "Dismiss"}
        </button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    /* Above AddTrackModal (200) and the Setup overlay (100) */
    z-index: 300;
    backdrop-filter: blur(4px);
  }

  .modal {
    width: min(520px, calc(100vw - 32px));
    max-height: calc(100vh - 64px);
    background: var(--bg-panel);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.5);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  h2 {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 16px;
    font-weight: 600;
    color: var(--fg);
  }

  .material-symbols-rounded {
    font-family: "Material Symbols Rounded Variable";
    font-size: 18px;
    line-height: 1;
  }

  .icon {
    color: var(--color-error);
    font-size: 20px;
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

  .close-btn:hover {
    color: var(--fg);
    background: var(--bg-button-hover);
  }

  .message {
    margin: 0;
    padding: 12px;
    max-height: 240px;
    overflow: auto;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 8px;
    color: var(--color-error);
    font-family: "JetBrains Mono", ui-monospace, monospace;
    font-size: 12px;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
  }

  .details {
    margin: 0;
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 4px 12px;
    font-size: 12px;
  }

  dt {
    color: var(--fg-muted);
  }

  dd {
    margin: 0;
    font-family: "JetBrains Mono", ui-monospace, monospace;
    word-break: break-all;
    user-select: text;
  }

  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--fg-muted);
  }

  footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
  }

  .remaining {
    margin-right: auto;
    font-size: 12px;
    color: var(--fg-muted);
  }
</style>
