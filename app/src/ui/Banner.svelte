<script lang="ts">
  import { app, dismissError, restartDaemon } from "../lib/state/app.svelte";
  import { connectionBanner } from "../lib/state/banner";

  const banner = $derived(connectionBanner(app.status, app.everConnected));
  const error = $derived(app.model.lastError);
</script>

{#if banner}
  <div class="banner">
    {banner.text}
    {#if banner.action}
      <button class="action" onclick={restartDaemon}>{banner.action}</button>
    {/if}
  </div>
{/if}
{#if error}
  <div class="banner error">
    {error.request}: {error.message}
    <button class="action" aria-label="Dismiss" onclick={dismissError}>×</button>
  </div>
{/if}

<style>
  .banner {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 12px;
    background: var(--jojo-banner);
    border-bottom: 1px solid var(--jojo-line);
    color: var(--jojo-fg2);
  }
  .banner.error {
    color: var(--jojo-error);
  }
  .action {
    margin-left: auto;
    padding: 2px 8px;
    border: 1px solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
  }
</style>
