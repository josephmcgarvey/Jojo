<script lang="ts">
  import type { ITheme } from "@xterm/xterm";
  import type { SessionInfo } from "../lib/protocol/SessionInfo";
  import { focusPane, renameSession } from "../lib/state/app.svelte";
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
        value={session.label}
        label="Session name"
        onCommit={(value) => {
          endRename();
          renameSession(session.id, value);
        }}
        onCancel={endRename}
      />
    {:else}
      <span class="label">{session.label}</span>
    {/if}
    {#if notice}
      <span class="notice" class:error={notice.tone === "error"} role="status">{notice.text}</span>
    {/if}
  </header>
  <div class="body">
    <TerminalPane {session} {terminalTheme} {focused} onmenu={showMenu} />
  </div>
</div>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    background: var(--jojo-pane);
    border: 1px solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
    overflow: hidden;
  }
  /* Spec 9.3 rule 4: the focused pane has a 1 px accent outline. */
  .pane.focused {
    border-color: var(--jojo-accent);
  }
  .strip {
    display: flex;
    align-items: center;
    gap: 7px;
    flex: none;
    height: 24px;
    padding: 0 10px;
    font-size: 11.5px;
    color: var(--jojo-fg2);
    background: var(--jojo-bar);
    border-bottom: 1px solid var(--jojo-line);
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
  .body {
    position: relative;
    flex: 1;
    min-height: 0;
    padding: 6px 8px;
  }
</style>
