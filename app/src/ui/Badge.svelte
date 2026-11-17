<script lang="ts">
  import type { AgentState } from "../lib/protocol/AgentState";

  let { agentState, size = 13, title }: { agentState: AgentState; size?: number; title?: string } = $props();
</script>

<!-- A shell session (state "running") has no hook-driven states, so it shows
     a terminal icon (a prompt chevron and cursor in a rounded frame) instead
     of a state circle. Every other state is a circle. -->
{#if agentState === "running"}
  <!-- An SVG shows its tooltip from a <title> child, not a title attribute. -->
  <svg class="terminal" width={size} height={size} viewBox="0 0 16 16" aria-hidden={title ? undefined : "true"} role={title ? "img" : undefined} aria-label={title}>
    {#if title}<title>{title}</title>{/if}
    <rect x="1.25" y="2.25" width="13.5" height="11.5" rx="2.5" fill="none" stroke="currentColor" stroke-width="1.5" />
    <path d="M4.5 6.25 6.75 8 4.5 9.75" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
    <path d="M8.5 10h3" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
  </svg>
{:else}
  <span class="badge {agentState}" {title} style:width="{size}px" style:height="{size}px"></span>
{/if}

<style>
  .terminal {
    display: inline-block;
    flex: none;
    color: var(--jojo-muted);
  }
  .badge {
    display: inline-block;
    flex: none;
    border-radius: var(--jojo-dot-radius);
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
