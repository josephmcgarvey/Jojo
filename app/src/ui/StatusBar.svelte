<script lang="ts">
  import { app } from "../lib/state/app.svelte";

  const project = $derived(app.selectedProjectId ? app.model.projects[app.selectedProjectId] : undefined);
</script>

<footer class="status">
  <span>This PC</span>
  {#if project}<span>{project.path}</span>{/if}
  <span class="grow"></span>
  {#if app.status.state === "connected"}<span>jojod {app.status.jojod_version}</span>{/if}
</footer>

<style>
  .status {
    display: flex;
    gap: 14px;
    padding: 5px 12px;
    font-size: 11px;
    color: var(--jojo-muted);
    background: var(--jojo-bar);
    border-top: 1px solid var(--jojo-line);
  }
  .grow {
    flex: 1;
  }
</style>
