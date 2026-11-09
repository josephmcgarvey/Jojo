<script lang="ts">
  import { activeTab, tabLabel } from "../lib/layout/tabs";
  import { countPanes } from "../lib/layout/tree";
  import { displayLabel, app, closeTab, currentProjectLayout, newSession, renameTab, selectTab } from "../lib/state/app.svelte";
  import { tabMenu } from "../lib/state/contextMenus";
  import { beginRename, endRename, openMenu, ui } from "../lib/state/ui.svelte";
  import { stateTooltip } from "../lib/state/toasts";
  import Badge from "./Badge.svelte";
  import PlusCircle from "./PlusCircle.svelte";
  import RenameField from "./RenameField.svelte";
  import { isMac } from "../lib/platform";
  import WindowControls from "./WindowControls.svelte";

  const project = $derived(currentProjectLayout());
  const activeId = $derived(project ? activeTab(project)?.id : undefined);
  const labelOf = (sessionId: string) => {
    const session = app.model.sessions[sessionId];
    return session && displayLabel(session);
  };

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
          {#if focusedSession}<Badge agentState={focusedSession.state} title={stateTooltip(focusedSession)} size={11} />{/if}
          <span class="name">{tabLabel(tab, labelOf)}</span>
          {#if panes > 1}<span class="count">{panes}</span>{/if}
        </button>
      {/if}
      <button class="close" aria-label="Close tab {tabLabel(tab, labelOf)}" onclick={() => void closeTab(tab.id)}>×</button>
    </div>
  {/each}
  {#if app.selectedProjectId}
    <button class="new" title="New terminal" aria-label="New terminal" onclick={() => newSession(app.selectedProjectId!, "shell")}><PlusCircle /></button>
  {/if}
  <div class="drag" data-tauri-drag-region></div>
  {#if !isMac}<WindowControls />{/if}
</div>

<style>
  /* Design pass 2.4: underline tabs over the window glow. */
  .tabs {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    height: 40px;
    padding: 6px 0 0 10px;
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 6px 0 12px;
    height: 32px;
    max-width: 260px;
    border-bottom: 2px solid transparent;
    color: var(--jojo-muted);
  }
  .tab.on {
    color: var(--jojo-fg);
    border-bottom-color: var(--jojo-accent);
    text-shadow: var(--jojo-tab-glow);
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
    display: flex;
    align-items: center;
    height: 32px;
    padding: 0 8px;
    color: var(--jojo-muted);
  }
  .new:hover {
    color: var(--jojo-fg);
  }
  .new:active {
    color: var(--jojo-accent);
  }
  .drag {
    flex: 1;
    align-self: stretch;
  }
</style>
