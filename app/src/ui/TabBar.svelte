<script lang="ts">
  import { activeTab, tabLabel } from "../lib/layout/tabs";
  import { countPanes } from "../lib/layout/tree";
  import { displayLabel, app, closeTab, currentProjectLayout, newSession, renameTab, reorderTab, selectTab } from "../lib/state/app.svelte";
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

  /**
   * Drag to reorder: a press on a tab that moves more than DRAG_THRESHOLD
   * pixels starts a drag, the pointer is captured so the drag continues
   * outside the tab, and the drop position is the number of tabs whose
   * center is left of the pointer. A shorter press is an ordinary click.
   * Pointer events are used instead of HTML5 drag and drop, which Tauri's
   * file-drop handling intercepts in the Windows webview.
   */
  const DRAG_THRESHOLD = 5;
  let tabsElement: HTMLDivElement;
  let press: { tabId: string; startX: number; pointerId: number; element: HTMLElement } | null = null;
  let dragging = $state<{ tabId: string; dropIndex: number } | null>(null);

  function dropIndexAt(x: number): number {
    const tabs = [...tabsElement.querySelectorAll<HTMLElement>("[data-tab-id]")];
    return tabs.filter((el) => {
      const r = el.getBoundingClientRect();
      return r.left + r.width / 2 < x;
    }).length;
  }

  function onTabPointerDown(event: PointerEvent, tabId: string): void {
    const target = event.target as HTMLElement;
    if (event.button !== 0 || target.closest(".close, input")) return;
    press = { tabId, startX: event.clientX, pointerId: event.pointerId, element: event.currentTarget as HTMLElement };
  }

  function onTabPointerMove(event: PointerEvent): void {
    if (!press || event.pointerId !== press.pointerId) return;
    if (!dragging) {
      if (Math.abs(event.clientX - press.startX) < DRAG_THRESHOLD) return;
      press.element.setPointerCapture(event.pointerId);
      selectTab(press.tabId);
    }
    dragging = { tabId: press.tabId, dropIndex: dropIndexAt(event.clientX) };
  }

  function onTabPointerUp(event: PointerEvent): void {
    if (!press || event.pointerId !== press.pointerId) return;
    if (dragging) {
      reorderTab(dragging.tabId, dragging.dropIndex);
      // The click that follows a drag must not act on the tab or button
      // under the pointer. It is dispatched right after pointerup, before
      // any timer, so the blocker is removed on the next turn; a drop that
      // produces no click (released outside the tab) leaves no blocker for
      // the user's next real click.
      const swallow = (e: MouseEvent) => e.stopPropagation();
      window.addEventListener("click", swallow, { capture: true, once: true });
      setTimeout(() => window.removeEventListener("click", swallow, { capture: true }), 0);
    }
    press = null;
    dragging = null;
  }

  function onTabPointerCancel(): void {
    press = null;
    dragging = null;
  }

  function onTabMenu(event: MouseEvent, tabId: string): void {
    event.preventDefault();
    selectTab(tabId);
    openMenu(event.clientX, event.clientY, tabMenu(tabId));
  }
</script>

<div class="tabs" bind:this={tabsElement}>
  {#each project?.tabs ?? [] as tab, index (tab.id)}
    {@const focusedSession = app.model.sessions[tab.focused]}
    {@const panes = countPanes(tab.root)}
    <!-- The tab's buttons are its keyboard controls; the container only adds the right-click menu. -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="tab"
      class:on={activeId === tab.id}
      class:dragging={dragging?.tabId === tab.id}
      class:drop-before={dragging !== null && dragging.dropIndex === index}
      class:drop-after={dragging !== null && index === (project?.tabs.length ?? 0) - 1 && dragging.dropIndex === index + 1}
      data-tab-id={tab.id}
      oncontextmenu={(e) => onTabMenu(e, tab.id)}
      onpointerdown={(e) => onTabPointerDown(e, tab.id)}
      onpointermove={onTabPointerMove}
      onpointerup={onTabPointerUp}
      onpointercancel={onTabPointerCancel}
    >
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
  .tab.dragging {
    opacity: 0.55;
  }
  /* The drop position: an accent bar on the left edge of the tab the
     dragged one will sit before, or the right edge of the last tab. */
  .tab.drop-before {
    box-shadow: inset 2px 0 0 var(--jojo-accent);
  }
  .tab.drop-after {
    box-shadow: inset -2px 0 0 var(--jojo-accent);
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
