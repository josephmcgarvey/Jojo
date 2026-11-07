<script lang="ts">
  import type { ITheme } from "@xterm/xterm";
  import type { SessionInfo } from "../lib/protocol/SessionInfo";
  import { focusPane } from "../lib/state/app.svelte";
  import Badge from "./Badge.svelte";
  import TerminalPane from "./TerminalPane.svelte";

  let { session, terminalTheme, focused }: { session: SessionInfo; terminalTheme: ITheme; focused: boolean } = $props();
</script>

<!-- Pointer focus only; keyboard users move focus between panes with Alt+Arrow (spec 9.6). -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="pane" class:focused onpointerdowncapture={() => focusPane(session.id)}>
  <header class="strip">
    <Badge agentState={session.state} size={10} />
    <span class="label">{session.label}</span>
  </header>
  <div class="body">
    <TerminalPane {session} {terminalTheme} {focused} />
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
  .body {
    position: relative;
    flex: 1;
    min-height: 0;
    padding: 6px 8px;
  }
</style>
