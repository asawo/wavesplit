<script lang="ts">
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";

  interface NavItem {
    id: string;
    label: string;
  }

  interface Props {
    activeId: string;
    onSelect: (id: string) => void;
    onImport: () => void;
  }

  let { activeId, onSelect, onImport }: Props = $props();

  const items: NavItem[] = [{ id: "library", label: "Library" }];

  let version = $state("");

  onMount(async () => {
    try {
      version = await getVersion();
    } catch {
      version = "";
    }
  });
</script>

<aside class="sidebar">
  <div class="brand">
    <h1>Wavesplit</h1>
    {#if version}
      <span class="version">v{version}</span>
    {/if}
  </div>
  <nav>
    {#each items as item (item.id)}
      <button
        type="button"
        class="nav-item"
        class:active={item.id === activeId}
        onclick={() => onSelect(item.id)}
      >
        <span class="icon" aria-hidden="true">
          <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
            <rect x="2" y="10" width="2" height="4" rx="1" />
            <rect x="6" y="7" width="2" height="10" rx="1" />
            <rect x="10" y="3" width="2" height="18" rx="1" />
            <rect x="14" y="6" width="2" height="12" rx="1" />
            <rect x="18" y="9" width="2" height="6" rx="1" />
          </svg>
        </span>
        <span class="label">{item.label}</span>
      </button>
    {/each}
  </nav>
  <div class="footer">
    <button type="button" class="btn add-track-btn" onclick={onImport}>
      <svg
        class="plus"
        viewBox="0 0 24 24"
        width="14"
        height="14"
        fill="none"
        stroke="currentColor"
        stroke-width="2.5"
        stroke-linecap="round"
        aria-hidden="true"
      >
        <line x1="12" y1="5" x2="12" y2="19" />
        <line x1="5" y1="12" x2="19" y2="12" />
      </svg>
      <span>Add Track</span>
    </button>
  </div>
</aside>

<style>
  .sidebar {
    width: 150px;
    flex-shrink: 0;
    background: var(--bg-sidebar);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    padding: 12px 0 0;
  }

  .brand {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    justify-content: center;
    min-height: 42px;
    padding: 0 20px;
    margin-bottom: 24px;
  }

  .version {
    font-family: "JetBrains Mono", ui-monospace, monospace;
    font-size: 10px;
    font-weight: 500;
    color: var(--fg-muted);
    letter-spacing: 0.04em;
    margin-top: 2px;
  }

  h1 {
    margin: 0;
    font-size: 28px;
    font-family: "Oleo Script Swash Caps", cursive;
    font-weight: 400;
    color: var(--color-ready);
    line-height: 1;
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 0 8px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 10px 12px 10px 14px;
    background: transparent;
    border: none;
    border-left: 3px solid transparent;
    color: var(--fg);
    font-size: 13px;
    font-family: inherit;
    text-align: left;
    cursor: pointer;
    border-radius: 0 6px 6px 0;
    transition:
      background 0.15s,
      color 0.15s;
  }

  .nav-item:hover:not(.active) {
    background: rgba(255, 255, 255, 0.04);
  }

  .nav-item.active {
    border-left-color: var(--color-ready);
    color: var(--color-ready);
    background: rgba(76, 175, 114, 0.08);
  }

  .icon {
    display: inline-flex;
    color: inherit;
  }

  .label {
    font-weight: 500;
  }

  .footer {
    margin-top: auto;
    padding: 12px;
    border-top: 1px solid var(--border);
  }

  .add-track-btn {
    justify-content: center;
    width: 100%;
    padding: 6px 10px;
    background: rgba(76, 175, 114, 0.14);
    border-color: var(--color-ready);
    color: var(--color-ready);
    font-weight: 500;
    transition:
      background 0.15s,
      transform 0.05s;
  }

  .add-track-btn:hover {
    background: var(--bg-button-hover);
  }

  .add-track-btn:active {
    transform: scale(0.98);
  }
</style>
