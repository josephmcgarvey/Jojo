<script lang="ts">
  // "Agents first" C: replaces the status bar. One segment per session that
  // has not exited, in sidebar order, grouped by location with a gap
  // between locations, colored by state (attention.ts). A segment's tooltip
  // names the session, its state and its location; a click jumps to it. The
  // summary on the right counts the states, most urgent first.
  import { app, displayLabel, focusPane, focusedSessionId, isFlagged } from "../lib/state/app.svelte";
  import { STATE_WORDS, stateSummary, stateToken, stripGroups } from "../lib/state/attention";
  import { orderedSessions } from "../lib/state/views";
  import Flag from "./Flag.svelte";

  const groups = $derived(stripGroups(orderedSessions(app.machines, app.models)));
  const summary = $derived(stateSummary(groups.flatMap((g) => g.sessions)));
  const focused = $derived(focusedSessionId());
  const labelOf = (machineId: string) => app.machines.find((m) => m.id === machineId)?.label ?? machineId;
</script>

<footer class="strip" aria-label="Every session's state">
  {#each groups as group (group.machineId)}
    <div class="group" title={labelOf(group.machineId)}>
      {#each group.sessions as session (session.id)}
        {@const token = stateToken(session.state)}
        {@const flagged = isFlagged(session.id)}
        <button
          class="seg"
          class:needs={session.state === "needs_you"}
          class:on={focused === session.id}
          style:--seg={token ? `var(${token})` : "var(--jojo-idle)"}
          title={`${displayLabel(session)} · ${STATE_WORDS[session.state]} · ${labelOf(group.machineId)}${flagged ? " · flagged" : ""}`}
          aria-label={`${displayLabel(session)}, ${STATE_WORDS[session.state]}, on ${labelOf(group.machineId)}${flagged ? ", flagged" : ""}`}
          onclick={() => focusPane(session.id)}
        >{#if flagged}<span class="pennant"><Flag size={9} /></span>{/if}</button>
      {/each}
    </div>
  {/each}
  <span class="grow"></span>
  {#if summary}<span class="summary">{summary}</span>{/if}
</footer>

<style>
  .strip {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: none;
    height: 26px;
    padding: 0 12px 4px 10px;
  }
  .group {
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
  }
  .seg {
    position: relative;
    flex: 0 1 56px;
    min-width: 12px;
    width: 56px;
    height: 8px;
    padding: 0;
    border-radius: 4px;
    background: var(--seg);
    opacity: 0.85;
  }
  .seg:hover,
  .seg:focus-visible {
    opacity: 1;
    outline: 1px solid var(--jojo-fg2);
    outline-offset: 2px;
  }
  /* The focused session's segment is taller, so it can be found in the strip. */
  .seg.on {
    height: 12px;
    opacity: 1;
  }
  .seg.needs {
    box-shadow: 0 0 8px var(--jojo-needs-you-glow);
  }
  /* A flagged session's segment carries the Flag for later pennant above
     its right end, since the sidebar rows that show flags can be hidden. */
  .pennant {
    position: absolute;
    right: -2px;
    bottom: 100%;
    display: flex;
    pointer-events: none;
  }
  .grow {
    flex: 1;
  }
  .summary {
    flex: none;
    font-size: 11.5px;
    color: var(--jojo-muted);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
</style>
