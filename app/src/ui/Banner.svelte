<script lang="ts">
  import { app, dismissError, restartDaemon } from "../lib/state/app.svelte";

  const status = $derived(app.status);
  const error = $derived(app.model.lastError);
</script>

{#if status.state === "starting"}
  <div class="banner">Starting jojod…</div>
{:else if status.state === "reconnecting"}
  <div class="banner">Disconnected from jojod: {status.message}. Reconnecting (attempt {status.attempt}). Sessions keep running.</div>
{:else if status.state === "elevated"}
  <div class="banner">Jojo is running as administrator, and jojod will not run elevated. Close Jojo and start it without administrator rights.</div>
{:else if status.state === "version_mismatch"}
  <div class="banner">
    {status.message}.
    <button class="action" onclick={restartDaemon}>Restart jojod (ends all running sessions)</button>
  </div>
{:else if status.state === "failed"}
  <div class="banner">
    Cannot reach jojod: {status.message}. Logs are in %LOCALAPPDATA%\Jojo\logs\jojod.&lt;date&gt;.log.
    <button class="action" onclick={restartDaemon}>Retry</button>
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
