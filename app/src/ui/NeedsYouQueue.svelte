<script lang="ts">
  // "Agents first" B: the sessions waiting on the user, from every location,
  // at the top of the sidebar, the longest wait first (attention.ts). Each
  // row shows what the session is asking (its state detail, for example the
  // permission prompt) and a click jumps to its pane. Hidden while nothing
  // is waiting.
  import { app, displayLabel, focusPane } from "../lib/state/app.svelte";
  import { needsYouQueue } from "../lib/state/attention";
  import { formatElapsed } from "../lib/state/elapsed";

  const queue = $derived(needsYouQueue(Object.values(app.model.sessions)));
</script>

{#if queue.length > 0}
  <section class="queue" aria-label="Sessions that need you">
    <h2><span>Needs you</span><span class="n">{queue.length}</span></h2>
    {#each queue as session (session.id)}
      <button class="ask" onclick={() => focusPane(session.id)} title={session.state_detail ?? "Waiting for your reply"}>
        <span class="dot" aria-hidden="true"></span>
        <span class="label">{displayLabel(session)}</span>
        <span class="time">{formatElapsed(app.now - session.state_changed_at)}</span>
        <span class="detail">{session.state_detail ?? "Waiting for your reply"}</span>
      </button>
    {/each}
  </section>
{/if}

<style>
  .queue {
    display: grid;
    gap: 4px;
    margin: 0 0 6px;
    padding: 8px;
    border-radius: var(--jojo-radius);
    background: color-mix(in srgb, var(--jojo-needs-you) 8%, transparent);
    border: 1px solid color-mix(in srgb, var(--jojo-needs-you) 30%, transparent);
  }
  h2 {
    display: flex;
    justify-content: space-between;
    margin: 0 0 2px;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--jojo-needs-you);
  }
  .n {
    font-variant-numeric: tabular-nums;
  }
  .ask {
    display: grid;
    grid-template-columns: 8px 1fr auto;
    align-items: center;
    gap: 2px 8px;
    width: 100%;
    padding: 6px 8px;
    border-radius: var(--jojo-radius-sm);
    background: var(--jojo-pane);
    text-align: left;
  }
  .ask:hover,
  .ask:focus-visible {
    background: var(--jojo-sel);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: var(--jojo-dot-radius);
    background: var(--jojo-needs-you);
    box-shadow: 0 0 8px var(--jojo-needs-you-glow);
  }
  .label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12.5px;
    font-weight: 500;
    color: var(--jojo-fg);
  }
  .time {
    font-size: 11px;
    color: var(--jojo-muted);
    font-variant-numeric: tabular-nums;
  }
  .detail {
    grid-column: 2 / 4;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11.5px;
    color: var(--jojo-muted);
  }
</style>
