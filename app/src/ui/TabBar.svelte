<script lang="ts">
  import { activeTab, tabLabel } from "../lib/layout/tabs";
  import { countPanes, paneIds } from "../lib/layout/tree";
  import { displayLabel, app, closeTab, currentProjectLayout, isFlagged, newSession, renameTab, reorderTab, selectTab } from "../lib/state/app.svelte";
  import { tabMenu } from "../lib/state/contextMenus";
  import { beginRename, endRename, openDialog, openMenu, ui } from "../lib/state/ui.svelte";
  import { stateTooltip } from "../lib/state/toasts";
  import { STATE_WORDS, stateToken } from "../lib/state/attention";
  import { formatElapsed } from "../lib/state/elapsed";
  import { SHORTCUT_LABELS } from "../lib/input/shortcuts";
  import Badge from "./Badge.svelte";
  import Flag from "./Flag.svelte";
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
            <!-- A tab shows the flag when any of its panes' sessions is flagged. -->
            {#if paneIds(tab.root).some((id) => isFlagged(id))}<Flag size={10} />{/if}
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
  <!-- Icon buttons for the snippets library (SnippetsDialog.svelte, a
       command-prompt icon) and the switcher ("Agents first" C, a magnifier;
       also Ctrl+Shift+P). The names are in the tooltips and aria-labels. -->
  <button class="tool" title="Snippets: saved commands to insert or run in the focused terminal" aria-label="Snippets" onclick={() => openDialog({ kind: "snippets" })}>
    <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
      <rect x="1.5" y="2.5" width="13" height="11" rx="2" fill="none" stroke="currentColor" stroke-width="1.3" />
      <path d="M4.5 6.2 6.8 8l-2.3 1.8M8.3 10.2h3.2" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
  </button>
  <button class="tool" title={`Search sessions, commands and output (${SHORTCUT_LABELS.switcher})`} aria-label="Search" onclick={() => openDialog({ kind: "switcher" })}>
    <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
      <circle cx="7" cy="7" r="4.6" fill="none" stroke="currentColor" stroke-width="1.4" />
      <path d="m10.4 10.4 3.4 3.4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
    </svg>
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
  /* The Snippets and Search icon buttons: 32 px squares in the pane color,
     with the theme's focus border and glow on hover and keyboard focus. */
  .tool {
    flex: none;
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border: var(--jojo-bw) solid var(--jojo-line);
    border-radius: var(--jojo-radius);
    background: var(--jojo-pane);
    color: var(--jojo-muted);
  }
  .tool + .tool {
    margin-left: -2px;
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
  /* Windows: the HTML window controls keep their 36 px row at the top. */
  .chrome {
    display: flex;
    align-self: flex-start;
    height: 36px;
    margin-top: -6px;
  }
</style>