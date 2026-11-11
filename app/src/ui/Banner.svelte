<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { app, dismissError, restartDaemon } from "../lib/state/app.svelte";
  import { connectionBanner, errorBannerText } from "../lib/state/banner";

  /** This PC's running sessions, read from state.json when its jojod speaks another protocol. */
  let runningSessions = $state<number | null>(null);
  /** Spec 8.4 "Later": the mismatch banner stays hidden until the status changes. */
  let deferred = $state(false);

  $effect(() => {
    if (app.status.state !== "version_mismatch") {
      deferred = false;
      return;
    }
    void invoke<number | null>("local_running_sessions")
      .then((count) => (runningSessions = count))
      .catch(() => (runningSessions = null));
  });

  const banner = $derived(deferred ? null : connectionBanner(app.status, app.everConnected, runningSessions));
  const error = $derived(app.model.lastError);
</script>

{#if banner}
  <div class="banner">
    {banner.text}
    {#if banner.action}
      <button class="action" onclick={() => restartDaemon()}>{banner.action}</button>
    {/if}
    {#if banner.later}
      <!-- The header keeps "restart required" while the banner is hidden. -->
      <button class="action" onclick={() => (deferred = true)}>Later</button>
    {/if}
  </div>
{/if}
{#if error}
  <div class="banner error">
    {errorBannerText(error)}
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
    border-bottom: var(--jojo-bw) solid var(--jojo-line);
    color: var(--jojo-fg2);
  }
  .banner.error {
    color: var(--jojo-error);
  }
  .action {
    margin-left: auto;
    padding: 2px 8px;
    border: var(--jojo-bw) solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
  }
</style>
