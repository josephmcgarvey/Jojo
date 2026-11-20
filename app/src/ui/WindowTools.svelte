<script lang="ts">
  // The sidebar button, Snippets and Search, kept together. The sidebar
  // shows them in its logo row; while the sidebar is hidden, the tab row
  // shows them at its left end, so the button that brings the sidebar back
  // is where the one that hid it was.
  import { SHORTCUT_LABELS } from "../lib/input/shortcuts";
  import { sidebarHidden, toggleSidebar } from "../lib/state/app.svelte";
  import { openDialog } from "../lib/state/ui.svelte";

  const hidden = $derived(sidebarHidden());
</script>

<div class="tools">
  <button
    class="tool"
    title={`${hidden ? "Show" : "Hide"} the sidebar (${SHORTCUT_LABELS["toggle-sidebar"]})`}
    aria-label={hidden ? "Show the sidebar" : "Hide the sidebar"}
    aria-pressed={!hidden}
    onclick={toggleSidebar}
  >
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <rect x="1.5" y="2.5" width="13" height="11" rx="2" fill="none" stroke="currentColor" stroke-width="1.3" />
      <path d="M6 2.5v11" fill="none" stroke="currentColor" stroke-width="1.3" />
      {#if !hidden}<path d="M2.5 4.5h2.5v7H2.5z" fill="currentColor" opacity="0.45" />{/if}
    </svg>
  </button>
  <button class="tool" title="Snippets: saved commands to insert or run in the focused terminal" aria-label="Snippets" onclick={() => openDialog({ kind: "snippets" })}>
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <rect x="1.5" y="2.5" width="13" height="11" rx="2" fill="none" stroke="currentColor" stroke-width="1.3" />
      <path d="M4.5 6.2 6.8 8l-2.3 1.8M8.3 10.2h3.2" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
  </button>
  <button class="tool" title={`Search sessions, commands and output (${SHORTCUT_LABELS.switcher})`} aria-label="Search" onclick={() => openDialog({ kind: "switcher" })}>
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <circle cx="7" cy="7" r="4.6" fill="none" stroke="currentColor" stroke-width="1.4" />
      <path d="m10.4 10.4 3.4 3.4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
    </svg>
  </button>
</div>

<style>
  .tools {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: none;
  }
  /* 28 px squares in the pane color, with the theme's focus border and glow
     on hover and keyboard focus. */
  .tool {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: var(--jojo-bw) solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
    background: var(--jojo-pane);
    color: var(--jojo-muted);
  }
  .tool:hover,
  .tool:focus-visible {
    color: var(--jojo-fg);
    border-color: var(--jojo-focus-border);
    box-shadow: var(--jojo-focus-ring);
    outline: none;
  }
  .tool:active {
    color: var(--jojo-accent);
  }
</style>
