<script lang="ts">
  import { activeTab, tabLabel } from "../lib/layout/tabs";
  import { countPanes } from "../lib/layout/tree";
  import { app, closeTab, currentProjectLayout, newSession, selectTab } from "../lib/state/app.svelte";
  import Badge from "./Badge.svelte";
  import WindowControls from "./WindowControls.svelte";

  const project = $derived(currentProjectLayout());
  const activeId = $derived(project ? activeTab(project)?.id : undefined);
  const labelOf = (sessionId: string) => app.model.sessions[sessionId]?.label;
</script>

<div class="tabs">
  {#each project?.tabs ?? [] as tab (tab.id)}
    {@const focusedSession = app.model.sessions[tab.focused]}
    {@const panes = countPanes(tab.root)}
    <div class="tab" class:on={activeId === tab.id}>
      <button class="title" onclick={() => selectTab(tab.id)}>
        {#if focusedSession}<Badge agentState={focusedSession.state} size={11} />{/if}
        <span class="name">{tabLabel(tab, labelOf)}</span>
        {#if panes > 1}<span class="count">{panes}</span>{/if}
      </button>
      <button class="close" aria-label="Close tab {tabLabel(tab, labelOf)}" onclick={() => void closeTab(tab.id)}>×</button>
    </div>
  {/each}
  {#if app.selectedProjectId}
    <button class="new" title="New Claude session" aria-label="New Claude session" onclick={() => newSession(app.selectedProjectId!, "claude")}>+</button>
  {/if}
  <div class="drag" data-tauri-drag-region></div>
  <WindowControls />
</div>

<style>
  .tabs {
    display: flex;
    align-items: flex-end;
    gap: 4px;
    height: 38px;
    padding: 6px 0 0 8px;
    background: var(--jojo-bar);
    border-bottom: 1px solid var(--jojo-line);
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 6px 0 12px;
    height: 31px;
    border-radius: var(--jojo-radius) var(--jojo-radius) 0 0;
    color: var(--jojo-muted);
  }
  .tab.on {
    background: var(--jojo-pane);
    color: var(--jojo-fg);
    border: 1px solid var(--jojo-line);
    border-bottom-color: transparent;
    margin-bottom: -1px;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 7px;
    max-width: 220px;
    white-space: nowrap;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .count {
    padding: 0 5px;
    border-radius: var(--jojo-radius-sm);
    font-size: 10.5px;
    color: var(--jojo-muted);
    background: var(--jojo-sel);
  }
  .close {
    padding: 0 4px;
    color: var(--jojo-muted);
  }
  .close:hover {
    color: var(--jojo-fg);
  }
  .new {
    height: 31px;
    padding: 0 10px;
    color: var(--jojo-muted);
  }
  .drag {
    flex: 1;
    align-self: stretch;
  }
</style>
