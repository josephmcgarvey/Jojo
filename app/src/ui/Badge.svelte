<script lang="ts">
  import type { AgentState } from "../lib/protocol/AgentState";

  let { agentState, size = 13, title }: { agentState: AgentState; size?: number; title?: string } = $props();
</script>

<span class="badge {agentState}" {title} style:width="{size}px" style:height="{size}px"></span>

<style>
  .badge {
    display: inline-block;
    flex: none;
    border-radius: 50%;
    border: 2px solid var(--jojo-idle);
  }
  .badge.working {
    border-color: var(--jojo-working);
    border-top-color: transparent;
    animation: spin 1s linear infinite;
  }
  .badge.needs_you {
    border-color: var(--jojo-needs-you);
    background: var(--jojo-needs-you-soft);
    box-shadow: 0 0 10px var(--jojo-needs-you-glow);
  }
  .badge.done {
    border-color: var(--jojo-done);
    background: var(--jojo-done);
  }
  .badge.error {
    border-color: var(--jojo-error);
    background: var(--jojo-error);
  }
  .badge.exited {
    opacity: 0.6;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .badge.working {
      animation: none;
    }
  }
</style>
