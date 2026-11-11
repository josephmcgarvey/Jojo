<script lang="ts">
  import { activeTab, tabLabel } from "../lib/layout/tabs";
  import { countPanes } from "../lib/layout/tree";
  import { displayLabel, app, closeTab, currentProjectLayout, newSession, renameTab, reorderTab, selectTab } from "../lib/state/app.svelte";
  import { tabMenu } from "../lib/state/contextMenus";
  import { beginRename, endRename, openDialog, openMenu, ui } from "../lib/state/ui.svelte";
  import { stateTooltip } from "../lib/state/toasts";
  import { STATE_WORDS, stateToken } from "../lib/state/attention";
  import { formatElapsed } from "../lib/state/elapsed";
  import { SHORTCUT_LABELS } from "../lib/input/shortcuts";
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

<!-- "Agents first" D: each tab is a card for its focused session: name,
     elapsed time in its state, the state word in its color, and what it is
     asking (state detail) or its project. Cards keep the tab behaviors:
     click selects, double-click renames, drag reorders, right-click opens
     the tab menu, × closes. -->
<div class="tabs" bind:this={tabsElement}>
  {#each project?.tabs ?? [] as tab, index (tab.id)}
    {@const focusedSession = app.model.sessions[tab.focused]}
    {@const panes = countPanes(tab.root)}
    {@const token = focusedSession ? stateToken(focusedSession.state) : null}
    <!-- The tab's buttons are its keyboard controls; the container only adds the right-click menu. -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="tab"
      class:on={activeId === tab.id}
      class:dragging={dragging?.tabId === tab.id}
      class:drop-before={dragging !== null && dragging.dropIndex === index}
      class:drop-after={dragging !== null && index === (project?.tabs.length ?? 0) - 1 && dragging.dropIndex === index + 1}
      style:--state={token ? `var(${token})` : "var(--jojo-muted)"}
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
          <span class="top">
            {#if focusedSession}<Badge agentState={focusedSession.state} title={stateTooltip(focusedSession)} size={11} />{/if}
            <span class="name">{tabLabel(tab, labelOf)}</span>
            {#if panes > 1}<span class="count" title="{panes} panes">{panes}</span>{/if}
            {#if focusedSession}<span class="time">{formatElapsed(app.now - focusedSession.state_changed_at)}</span>{/if}
          </span>
          {#if focusedSession}
            <span class="meta">
              <span class="state">{STATE_WORDS[focusedSession.state]}</span>
              <span class="detail">{focusedSession.state_detail ?? app.model.projects[focusedSession.project_id]?.name ?? ""}</span>
            </span>
          {/if}
        </button>
      {/if}
      <button class="close" aria-label="Close tab {tabLabel(tab, labelOf)}" onclick={() => void closeTab(tab.id)}>×</button>
    </div>
  {/each}
  {#if app.selectedProjectId}
    <button class="new" title="New terminal" aria-label="New terminal" onclick={() => newSession(app.selectedProjectId!, "shell")}><PlusCircle /></button>
  {/if}
  <div class="drag" data-tauri-drag-region></div>
  <!-- "Agents first" C: the switcher, reached by click as well as by its shortcut. -->
  <button class="search" onclick={() => openDialog({ kind: "switcher" })}>
    <span>Search sessions and commands</span><kbd>{SHORTCUT_LABELS.switcher}</kbd>
  </button>
  {#if !isMac}<div class="chrome"><WindowControls /></div>{/if}
</div>

<style>
  .tabs {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 64px;
    padding: 6px 0 4px 10px;
  }
  .tab {
    position: relative;
    display: flex;
    align-items: stretch;
    flex: 0 1 210px;
    min-width: 120px;
    height: 50px;
    border-radius: var(--jojo-radius);
    background: var(--jojo-pill);
    box-shadow: var(--jojo-panel-shadow);
    border: var(--jojo-bw) solid var(--jojo-line);
    color: var(--jojo-fg2);
  }
  .tab.on {
    color: var(--jojo-fg);
    border-color: var(--jojo-focus-border);
    box-shadow: var(--jojo-focus-ring);
  }
  .tab.dragging {
    opacity: 0.55;
  }
  /* The drop position: an accent bar on the left edge of the card the
     dragged one will sit before, or the right edge of the last card. */
  .tab.drop-before {
    box-shadow: inset 3px 0 0 var(--jojo-accent);
  }
  .tab.drop-after {
    box-shadow: inset -3px 0 0 var(--jojo-accent);
  }
  .title {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 2px;
    flex: 1;
    min-width: 0;
    padding: 0 4px 0 10px;
    text-align: left;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    font-size: 12.5px;
    font-weight: 500;
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    flex: none;
    padding: 0 5px;
    border-radius: var(--jojo-radius-sm);
    font-size: 10.5px;
    color: var(--jojo-muted);
    background: var(--jojo-sel);
  }
  .time {
    flex: none;
    font-size: 11px;
    font-weight: 400;
    color: var(--jojo-muted);
    font-variant-numeric: tabular-nums;
  }
  .meta {
    display: flex;
    gap: 6px;
    min-width: 0;
    font-size: 11px;
    white-space: nowrap;
  }
  .state {
    flex: none;
    color: var(--state);
    font-size: 10.5px;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }
  .detail {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--jojo-muted);
  }
  .close {
    align-self: flex-start;
    padding: 4px 7px;
    color: var(--jojo-muted);
  }
  .close:hover {
    color: var(--jojo-fg);
  }
  .new {
    display: flex;
    align-items: center;
    height: 50px;
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
  .search {
    flex: none;
    display: flex;
    align-items: center;
    gap: 12px;
    height: 32px;
    padding: 0 10px;
    border: var(--jojo-bw) solid var(--jojo-line);
    border-radius: var(--jojo-radius);
    background: var(--jojo-pane);
    color: var(--jojo-muted);
    font-size: 12.5px;
  }
  .search:hover,
  .search:focus-visible {
    color: var(--jojo-fg2);
    border-color: var(--jojo-focus-border);
  }
  .search kbd {
    font-family: var(--jojo-font-mono);
    font-size: 11px;
    color: var(--jojo-fg2);
  }
  /* Windows: the HTML window controls keep their 36 px row at the top. */
  .chrome {
    display: flex;
    align-self: flex-start;
    height: 36px;
    margin-top: -6px;
  }
</style>