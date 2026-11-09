<script lang="ts">
  import type { ITheme } from "@xterm/xterm";
  import type { SessionInfo } from "../lib/protocol/SessionInfo";
  import { app, displayLabel, focusPane, machineOfSession, renameSession } from "../lib/state/app.svelte";
  import { disconnectedOverlay } from "../lib/state/machines";
  import { paneMenu } from "../lib/state/contextMenus";
  import { notices } from "../lib/state/notices.svelte";
  import { endRename, openMenu, ui } from "../lib/state/ui.svelte";
  import { stateTooltip } from "../lib/state/toasts";
  import Badge from "./Badge.svelte";
  import RenameField from "./RenameField.svelte";
  import TerminalPane from "./TerminalPane.svelte";

  let { session, terminalTheme, focused }: { session: SessionInfo; terminalTheme: ITheme; focused: boolean } = $props();

  const renaming = $derived(ui.renaming?.kind === "session" && ui.renaming.where === "pane" && ui.renaming.sessionId === session.id);
  const notice = $derived(notices[session.id]);
  const project = $derived(app.model.projects[session.project_id]);
  const machineId = $derived(machineOfSession(session.id));
  const overlay = $derived(disconnectedOverlay(app.machines.find((m) => m.id === machineId), app.statuses[machineId]));

  function showMenu(x: number, y: number): void {
    focusPane(session.id);
    openMenu(x, y, paneMenu(session.id));
  }

  function onHeaderMenu(event: MouseEvent): void {
    event.preventDefault();
    showMenu(event.clientX, event.clientY);
  }
</script>

<!-- Pointer focus only; keyboard users move focus between panes with Alt+Arrow (spec 9.6). -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="pane" class:focused onpointerdowncapture={() => focusPane(session.id)}>
  <!-- Spec 9.4: right-click on the header strip opens the pane menu, whatever the mouse mode. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <header class="strip" oncontextmenu={onHeaderMenu}>
    <Badge agentState={session.state} title={stateTooltip(session)} size={10} />
    {#if renaming}
      <RenameField
        value={displayLabel(session)}
        label="Session name"
        onCommit={(value) => {
          endRename();
          renameSession(session.id, value);
        }}
        onCancel={endRename}
      />
    {:else}
      <span class="label">{displayLabel(session)}</span>
    {/if}
    {#if notice}
      <span class="notice" class:error={notice.tone === "error"} role="status">{notice.text}</span>
    {:else if project}
      <span class="project">{project.name}</span>
    {/if}
  </header>
  <div class="body">
    <TerminalPane {session} {terminalTheme} {focused} onmenu={showMenu} />
    {#if overlay}
      <!-- Spec 8.3 rules 2 and 4: keystrokes are discarded while the machine
           is disconnected (the backend has no connection to send them on). -->
      <div class="overlay" role="status">{overlay}</div>
    {/if}
  </div>
</div>

<style>
  /* Design pass 1.3: frosted cards; the terminal inside stays opaque. */
  .pane {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    background: var(--jojo-glass-pane);
    backdrop-filter: blur(10px);
    border: 1px solid var(--jojo-line);
    border-radius: var(--jojo-radius-lg);
    overflow: hidden;
  }
  /* Spec 9.3 rule 4, design pass 1.3: the focused pane's accent border and glow. */
  .pane.focused {
    border-color: var(--jojo-focus-border);
    box-shadow: var(--jojo-focus-ring);
  }
  .strip {
    display: flex;
    align-items: center;
    gap: 7px;
    flex: none;
    height: 28px;
    padding: 0 14px;
    font-size: 10.5px;
    color: var(--jojo-muted);
  }
  .pane.focused .strip {
    color: var(--jojo-fg2);
  }
  .project {
    flex: none;
    color: var(--jojo-muted);
  }
  .label {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .notice {
    flex: none;
    max-width: 60%;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--jojo-done);
  }
  .notice.error {
    color: var(--jojo-error);
  }
  .overlay {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 16px;
    text-align: center;
    background: var(--jojo-scrim);
    color: var(--jojo-fg2);
    font-family: var(--jojo-font-ui);
    font-size: 12.5px;
  }
  .body {
    position: relative;
    flex: 1;
    min-height: 0;
    padding: 2px 10px 10px;
  }
</style>
