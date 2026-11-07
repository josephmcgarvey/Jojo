<script lang="ts">
  import { activeTab, tabLabel } from "../lib/layout/tabs";
  import { countPanes } from "../lib/layout/tree";
  import { app, closeTab, currentProjectLayout, newSession, renameTab, selectTab } from "../lib/state/app.svelte";
  import { tabMenu } from "../lib/state/contextMenus";
  import { beginRename, endRename, openMenu, ui } from "../lib/state/ui.svelte";
  import Badge from "./Badge.svelte";
  import RenameField from "./RenameField.svelte";
  import WindowControls from "./WindowControls.svelte";

  const project = $derived(currentProjectLayout());
  const activeId = $derived(project ? activeTab(project)?.id : undefined);
  const labelOf = (sessionId: string) => app.model.sessions[sessionId]?.label;

  function onTabMenu(event: MouseEvent, tabId: string): void {
    event.preventDefault();
    selectTab(tabId);
    openMenu(event.clientX, event.clientY, tabMenu(tabId));
  }
</script>

<div class="tabs">
  {#each project?.tabs ?? [] as tab (tab.id)}
    {@const focusedSession = app.model.sessions[tab.focused]}
    {@const panes = countPanes(tab.root)}
    <!-- The tab's buttons are its keyboard controls; the container only adds the right-click menu. -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="tab" class:on={activeId === tab.id} oncontextmenu={(e) => onTabMenu(e, tab.id)}>
      {#if ui.renaming?.kind === "tab" && ui.renaming.tabId === tab.id}
        <RenameField
          value={tabLabel(tab, labelOf)}
          label="Tab name"
          onCommit={(value) => {
            endRename();
            renameTab(tab.id, value);
          }}
          onCancel={endRename}
        />
      {:else}
        <button class="title" onclick={() => selectTab(tab.id)} ondblclick={() => beginRename({ kind: "tab", tabId: tab.id })}>
          {#if focusedSession}<Badge agentState={focusedSession.state} size={11} />{/if}
          <span class="name">{tabLabel(tab, labelOf)}</span>
          {#if panes > 1}<span class="count">{panes}</span>{/if}
        </button>
      {/if}
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
    max-width: 260px;
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
    min-width: 0;
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
