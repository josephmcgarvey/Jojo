<script lang="ts">
  import { onMount } from "svelte";
  import { app, currentTab, focusedSessionId, startApp } from "./lib/state/app.svelte";
  import { closeDialog, closeMenu, ui } from "./lib/state/ui.svelte";
  import { resolveShortcut } from "./lib/input/shortcuts";
  import { runAction } from "./lib/state/actions";
  import { dialog } from "./lib/state/dialog.svelte";
  import { terminalControls } from "./lib/terminal/registry";
  import { applyTheme, resolveTheme } from "./lib/theme";
  import Banner from "./ui/Banner.svelte";
  import ConfirmDialog from "./ui/ConfirmDialog.svelte";
  import ContextMenu from "./ui/ContextMenu.svelte";
  import AddMachineDialog from "./ui/AddMachineDialog.svelte";
  import AddProjectDialog from "./ui/AddProjectDialog.svelte";
  import SettingsDialog from "./ui/SettingsDialog.svelte";
  import SwitcherDialog from "./ui/SwitcherDialog.svelte";
  import PaneGrid from "./ui/PaneGrid.svelte";
  import Sidebar from "./ui/Sidebar.svelte";
  import StatusBar from "./ui/StatusBar.svelte";
  import TabBar from "./ui/TabBar.svelte";

  // The theme follows the saved appearance settings, or the Settings
  // screen's unsaved choices while it is open (live preview).
  const theme = $derived(resolveTheme(app.themePreview ?? app.settings));
  $effect(() => applyTheme(theme));
  onMount(() => {
    window.addEventListener("keydown", onKeydown, true);
    const stopApp = startApp();
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

  /** Closing a menu returns keyboard focus to the focused pane's terminal. */
  function closeMenuAndRefocus(): void {
    closeMenu();
    terminalControls(focusedSessionId())?.focus();
  }
</script>

<div class="window">
  <Sidebar />
  <section class="main">
    <TabBar />
    <Banner />
    <div class="panes">
      {#if tab}
        <PaneGrid terminalTheme={theme.terminal} />
      {:else if app.selectedProjectId}
        <p class="empty">No session open. Use + next to the project to open a terminal.</p>
      {:else}
        <p class="empty">Add a project folder with + next to This PC.</p>
      {/if}
    </div>
    <StatusBar />
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
    display: grid;
    grid-template-columns: 268px 1fr;
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
</style>
