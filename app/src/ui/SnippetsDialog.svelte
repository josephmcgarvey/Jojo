<script lang="ts">
  // The snippets library (the Snippets button in the tab row). It lists the
  // snippets for the focused terminal's location and project, grouped by
  // folder. A click on a snippet types it into that terminal without
  // pressing Enter; Run types it and presses Enter; Copy puts it on the
  // clipboard. Up and Down move the highlight, Enter inserts, Ctrl+Enter
  // (⌘Enter on macOS) runs, Esc closes.
  import { onMount, tick } from "svelte";
  import type { Snippet } from "../lib/backend/Snippet";
  import { isMac } from "../lib/platform";
  import { app, focusedSessionId } from "../lib/state/app.svelte";
  import { copySnippet, insertSnippet, snippets, snippetTarget, snippetTargetLabel } from "../lib/state/snippets.svelte";
  import { openDialog, ui } from "../lib/state/ui.svelte";
  import { appliesTo, groupSnippets, matchesQuery } from "../lib/snippets/library";
  import { terminalControls } from "../lib/terminal/registry";

  let { onclose }: { onclose: () => void } = $props();

  let query = $state("");
  let showAll = $state(false);
  let highlight = $state(0);
  let input: HTMLInputElement;

  const target = snippetTarget();
  const targetLabel = snippetTargetLabel();
  const canInsert = target !== null && terminalControls(focusedSessionId()) !== undefined;

  const groups = $derived(groupSnippets(snippets.list.filter((s) => (showAll || appliesTo(s.scope, target)) && matchesQuery(s, query))));
  /** The listed snippets in display order, for the keyboard highlight. */
  const flat = $derived(groups.flatMap((g) => g.snippets));
  const hiddenCount = $derived(snippets.list.filter((s) => !appliesTo(s.scope, target)).length);

  $effect(() => {
    void query;
    void showAll;
    highlight = 0;
  });

  onMount(() => input.focus());

  function scopeLabel(snippet: Snippet): string | null {
    const scope = snippet.scope;
    if (scope.kind === "machine") return app.machines.find((m) => m.id === scope.machine_id)?.label ?? "Removed location";
    if (scope.kind === "project") return app.model.projects[scope.project_id]?.name ?? "Removed project";
    return null;
  }

  function use(snippet: Snippet | undefined, run: boolean): void {
    if (!snippet || !canInsert) return;
    onclose();
    insertSnippet(snippet.id, run);
  }

  function edit(snippet: Snippet | null): void {
    openDialog({ kind: "snippet-edit", snippetId: snippet?.id ?? null });
  }

  function close(): void {
    onclose();
    void tick().then(() => {
      if (!ui.dialog && !ui.menu) terminalControls(focusedSessionId())?.focus();
    });
  }

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      close();
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      highlight = Math.max(0, Math.min(flat.length - 1, highlight + 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      highlight = Math.max(0, highlight - 1);
    } else if (event.key === "Enter" && event.target === input) {
      event.preventDefault();
      use(flat[highlight], isMac ? event.metaKey : event.ctrlKey);
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="scrim" onclick={close} onkeydown={onKeydown}>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="panel" role="dialog" aria-modal="true" aria-label="Snippets" tabindex="-1" onclick={(e) => e.stopPropagation()}>
    <div class="top">
      <input bind:this={input} bind:value={query} placeholder="Find a snippet" spellcheck="false" aria-label="Find a snippet" />
      <button class="new" onclick={() => edit(null)}>New snippet</button>
    </div>
    <p class="target">
      {#if canInsert}
        Inserts into <strong>{targetLabel}</strong>. Click to insert, <kbd>{isMac ? "⌘" : "Ctrl+"}Enter</kbd> or Run to run.
      {:else}
        Focus a terminal to insert snippets. You can still copy and edit them.
      {/if}
    </p>
    <ul aria-label="Snippets">
      {#each groups as group (group.folder)}
        {#if group.folder}<li class="folder" role="presentation">{group.folder}</li>{/if}
        {#each group.snippets as snippet (snippet.id)}
          {@const index = flat.indexOf(snippet)}
          {@const scope = scopeLabel(snippet)}
          <li class="row" class:on={index === highlight}>
            <button class="main" disabled={!canInsert} title={canInsert ? "Insert into the terminal without running it" : "No terminal is focused"} onmouseenter={() => (highlight = index)} onclick={() => use(snippet, false)}>
              <span class="name">{snippet.name}{#if scope}<span class="scope">{scope}</span>{/if}</span>
              <span class="command">{snippet.command}</span>
            </button>
            <span class="tools">
              <button disabled={!canInsert} onclick={() => use(snippet, true)} title="Insert and press Enter">Run</button>
              <button onclick={() => void copySnippet(snippet.id)} title="Copy to the clipboard">Copy</button>
              <button onclick={() => edit(snippet)} title="Edit or delete">Edit</button>
            </span>
          </li>
        {/each}
      {:else}
        <li class="empty">
          {#if snippets.list.length === 0}
            No snippets yet. Click New snippet, or select text in a terminal, right-click it and choose Save as snippet.
          {:else if query}
            No snippet matches “{query}”.
          {:else}
            No snippets for this terminal.
          {/if}
        </li>
      {/each}
    </ul>
    {#if hiddenCount > 0}
      <label class="all"><input type="checkbox" bind:checked={showAll} /> Show {hiddenCount} snippet{hiddenCount === 1 ? "" : "s"} for other locations and projects</label>
    {/if}
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 200;
    display: flex;
    justify-content: center;
    padding-top: 12vh;
    background: var(--jojo-scrim);
  }
  .panel {
    width: min(620px, calc(100% - 32px));
    max-height: 66vh;
    display: flex;
    flex-direction: column;
    background: var(--jojo-menu);
    border: var(--jojo-bw) solid var(--jojo-line);
    border-radius: var(--jojo-radius);
    box-shadow: var(--jojo-shadow);
    overflow: hidden;
    outline: none;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-right: 10px;
    border-bottom: var(--jojo-bw) solid var(--jojo-line);
  }
  .top input {
    flex: 1;
    min-width: 0;
    padding: 10px 14px;
    border: none;
    background: transparent;
    color: var(--jojo-fg);
    font: inherit;
    font-size: 14px;
    outline: none;
  }
  .new {
    flex: none;
    padding: 4px 10px;
    border: var(--jojo-bw) solid var(--jojo-accent);
    border-radius: var(--jojo-radius-sm);
    background: var(--jojo-accent);
    color: var(--jojo-bg);
    font-weight: 500;
  }
  .target {
    margin: 0;
    padding: 6px 14px;
    color: var(--jojo-muted);
    font-size: 12px;
    border-bottom: var(--jojo-bw) solid var(--jojo-line);
  }
  .target strong {
    color: var(--jojo-fg2);
    font-weight: 500;
  }
  kbd {
    font-family: var(--jojo-font-mono);
    font-size: 11px;
    color: var(--jojo-fg2);
  }
  ul {
    margin: 0;
    padding: 4px;
    list-style: none;
    overflow-y: auto;
  }
  .folder {
    padding: 8px 10px 3px;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--jojo-muted);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
    border-radius: var(--jojo-radius-sm);
  }
  .row.on {
    background: var(--jojo-sel);
  }
  .main {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 1px;
    padding: 6px 10px;
    text-align: left;
    color: var(--jojo-fg2);
  }
  .row.on .main {
    color: var(--jojo-fg);
  }
  .main:disabled {
    cursor: default;
  }
  .name {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .scope {
    flex: none;
    padding: 0 6px;
    border-radius: var(--jojo-radius-sm);
    background: var(--jojo-pill);
    color: var(--jojo-muted);
    font-size: 10.5px;
    font-weight: 400;
  }
  .command {
    font-family: var(--jojo-font-mono);
    font-size: 11.5px;
    color: var(--jojo-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tools {
    flex: none;
    display: flex;
    gap: 4px;
    padding-right: 6px;
    opacity: 0;
  }
  .row.on .tools,
  .row:focus-within .tools {
    opacity: 1;
  }
  .tools button {
    padding: 2px 8px;
    border: var(--jojo-bw) solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
    color: var(--jojo-fg2);
    font-size: 11.5px;
  }
  .tools button:hover:not(:disabled) {
    color: var(--jojo-fg);
    border-color: var(--jojo-focus-border);
  }
  .tools button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .empty {
    padding: 14px 10px;
    color: var(--jojo-muted);
    font-size: 12px;
  }
  .all {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 14px;
    border-top: var(--jojo-bw) solid var(--jojo-line);
    color: var(--jojo-muted);
    font-size: 12px;
  }
</style>
