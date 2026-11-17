<script lang="ts">
  import { onMount } from "svelte";
  import { app, currentTab, focusedSessionId, saveSidebarWidth, startApp } from "./lib/state/app.svelte";
  import { closeDialog, closeMenu, ui } from "./lib/state/ui.svelte";
  import { resolveShortcut } from "./lib/input/shortcuts";
  import { runAction } from "./lib/state/actions";
  import { dialog } from "./lib/state/dialog.svelte";
  import { terminalControls } from "./lib/terminal/registry";
  import { LOCAL_LABEL } from "./lib/platform";
  import { clampSidebarWidth, DEFAULT_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH, MIN_SIDEBAR_WIDTH, SIDEBAR_KEY_STEP } from "./lib/layout/sidebar";
  import { applyTheme, resolveTheme } from "./lib/theme";
  import Banner from "./ui/Banner.svelte";
  import ConfirmDialog from "./ui/ConfirmDialog.svelte";
  import ContextMenu from "./ui/ContextMenu.svelte";
  import AddMachineDialog from "./ui/AddMachineDialog.svelte";
  import AddProjectDialog from "./ui/AddProjectDialog.svelte";
  import SettingsDialog from "./ui/SettingsDialog.svelte";
  import SwitcherDialog from "./ui/SwitcherDialog.svelte";
  import SnippetsDialog from "./ui/SnippetsDialog.svelte";
  import SnippetEditDialog from "./ui/SnippetEditDialog.svelte";
  import SnippetFillDialog from "./ui/SnippetFillDialog.svelte";
  import ImportConfigDialog from "./ui/ImportConfigDialog.svelte";
  import { loadSnippets } from "./lib/state/snippets.svelte";
  import PaneGrid from "./ui/PaneGrid.svelte";
  import Sidebar from "./ui/Sidebar.svelte";
  import StateStrip from "./ui/StateStrip.svelte";
  import TabBar from "./ui/TabBar.svelte";

  // The theme follows the saved appearance settings, or the Settings
  // screen's unsaved choices while it is open (live preview).
  const theme = $derived(resolveTheme(app.themePreview ?? app.settings));
  $effect(() => applyTheme(theme));
  onMount(() => {
    window.addEventListener("keydown", onKeydown, true);
    const stopApp = startApp();
    loadSnippets();
    return () => {
      window.removeEventListener("keydown", onKeydown, true);
      stopApp();
    };
  });
  /**
   * Application shortcuts (spec 9.6), handled in the capture phase on window
   * so they run before xterm.js's keydown handler on its textarea. A matched
   * shortcut is stopped here, so xterm.js never sends it to the program;
   * every other key reaches the terminal unchanged. The dialog, menus and
   * rename fields handle their own keys.
   */
  function onKeydown(event: KeyboardEvent): void {
    if (dialog.confirm || ui.menu || ui.dialog || event.target instanceof HTMLInputElement) return;
    const action = resolveShortcut(event, {
      copyOnSelect: app.settings?.copyOnSelect ?? true,
      hasSelection: terminalControls(focusedSessionId())?.hasSelection() ?? false,
    });
    if (!action) return;
    event.preventDefault();
    event.stopPropagation();
    runAction(action);
  }
  const tab = $derived(currentTab());

  /**
   * The sidebar's width: while its right edge is dragged, the dragged
   * width; otherwise the sidebarWidth setting. Dragging changes the width
   * live (the panes refit through their resize observers) and saves it on
   * release; a double-click returns to the default; Left and Right on the
   * focused edge move it by SIDEBAR_KEY_STEP.
   */
  let dragWidth = $state<number | null>(null);
  const sidebarWidth = $derived(dragWidth ?? clampSidebarWidth(app.settings?.sidebarWidth));
  let resizeStart: { x: number; width: number; pointerId: number } | null = null;

  function onResizeDown(event: PointerEvent): void {
    if (event.button !== 0) return;
    event.preventDefault();
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    resizeStart = { x: event.clientX, width: sidebarWidth, pointerId: event.pointerId };
    dragWidth = sidebarWidth;
  }

  function onResizeMove(event: PointerEvent): void {
    if (!resizeStart || event.pointerId !== resizeStart.pointerId) return;
    dragWidth = clampSidebarWidth(resizeStart.width + event.clientX - resizeStart.x);
  }

  function onResizeUp(event: PointerEvent): void {
    if (!resizeStart || event.pointerId !== resizeStart.pointerId) return;
    const width = dragWidth ?? sidebarWidth;
    resizeStart = null;
    // The setting takes the new width before the dragged value is dropped, so the edge does not jump back.
    void saveSidebarWidth(width).finally(() => (dragWidth = null));
  }

  function onResizeKey(event: KeyboardEvent): void {
    const step = event.key === "ArrowLeft" ? -SIDEBAR_KEY_STEP : event.key === "ArrowRight" ? SIDEBAR_KEY_STEP : 0;
    if (step === 0) return;
    event.preventDefault();
    void saveSidebarWidth(clampSidebarWidth(sidebarWidth + step));
  }

  /** Closing a menu returns keyboard focus to the focused pane's terminal. */
  function closeMenuAndRefocus(): void {
    closeMenu();
    terminalControls(focusedSessionId())?.focus();
  }
</script>

<div class="window" class:resizing={dragWidth !== null} style:--side-w={`${sidebarWidth}px`}>
  <Sidebar />
  <!-- A focusable separator is a widget in ARIA (it has a value and takes
       Left and Right), which Svelte's check does not know. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    class="resizer"
    role="separator"
    aria-orientation="vertical"
    aria-label="Sidebar width"
    aria-valuenow={sidebarWidth}
    aria-valuemin={MIN_SIDEBAR_WIDTH}
    aria-valuemax={MAX_SIDEBAR_WIDTH}
    tabindex="0"
    title="Drag to resize the sidebar; double-click for the default width"
    onpointerdown={onResizeDown}
    onpointermove={onResizeMove}
    onpointerup={onResizeUp}
    onpointercancel={onResizeUp}
    ondblclick={() => void saveSidebarWidth(DEFAULT_SIDEBAR_WIDTH)}
    onkeydown={onResizeKey}
  ></div>
  <section class="main">
    <TabBar />
    <Banner />
    <div class="panes">
      {#if tab}
        <PaneGrid terminalTheme={theme.terminal} />
      {:else if app.selectedProjectId}
        <p class="empty">No session open. Use + next to the project to open a terminal.</p>
      {:else}
        <p class="empty">Add a project folder with + next to {LOCAL_LABEL}.</p>
      {/if}
    </div>
    <StateStrip />
  </section>
</div>
{#if ui.dialog?.kind === "add-machine"}
  <AddMachineDialog onclose={closeDialog} />
{:else if ui.dialog?.kind === "add-project"}
  <AddProjectDialog machineId={ui.dialog.machineId} onclose={closeDialog} />
{:else if ui.dialog?.kind === "settings"}
  <SettingsDialog onclose={closeDialog} />
{:else if ui.dialog?.kind === "switcher"}
  <SwitcherDialog onclose={closeDialog} />
{:else if ui.dialog?.kind === "snippets"}
  <SnippetsDialog onclose={closeDialog} />
{:else if ui.dialog?.kind === "snippet-edit"}
  <SnippetEditDialog snippetId={ui.dialog.snippetId} command={ui.dialog.command ?? ""} onclose={closeDialog} />
{:else if ui.dialog?.kind === "snippet-fill"}
  <SnippetFillDialog snippetId={ui.dialog.snippetId} run={ui.dialog.run} onclose={closeDialog} />
{:else if ui.dialog?.kind === "import-config"}
  <ImportConfigDialog path={ui.dialog.path} preview={ui.dialog.preview} onclose={closeDialog} />
{/if}
<!-- After the dialogs, so a confirmation asked from one (Settings ▸ Restart
     daemon) paints above it: both scrims share z-index 200. -->
<ConfirmDialog />
{#if ui.menu}
  {#key ui.menu}
    <ContextMenu entries={ui.menu.entries} x={ui.menu.x} y={ui.menu.y} onclose={closeMenuAndRefocus} />
  {/key}
{/if}

<style>
  .window {
    position: relative;
    display: grid;
    grid-template-columns: var(--side-w) 1fr;
    height: 100%;
    background: var(--jojo-bg-glow);
  }
  .main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .panes {
    position: relative;
    flex: 1;
    min-height: 0;
  }
  .empty {
    margin: 40px auto;
    text-align: center;
    color: var(--jojo-muted);
  }
  /* The sidebar's right edge: an 8 px handle over the gap between the
     sidebar and the panes, with an accent line while hovered, focused or
     dragged. Positioned, not a grid column, so it takes no width. */
  .resizer {
    position: absolute;
    top: 0;
    bottom: 0;
    left: calc(var(--side-w) - 2px);
    z-index: 5;
    width: 8px;
    cursor: col-resize;
    outline: none;
  }
  .resizer::after {
    content: "";
    position: absolute;
    top: 12px;
    bottom: 12px;
    left: 3px;
    width: 2px;
    border-radius: 1px;
    background: transparent;
    transition: background 120ms;
  }
  .resizer:hover::after,
  .resizer:focus-visible::after,
  .resizing .resizer::after {
    background: var(--jojo-accent);
  }
  /* While dragging, the cursor stays a resize cursor and text is not selected. */
  .resizing {
    cursor: col-resize;
    user-select: none;
  }
</style>
