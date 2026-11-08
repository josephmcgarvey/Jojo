<script lang="ts">
  // Spec 9.7: the focused session's machine label and project path (the
  // selected project's when no pane is focused), and the switcher hint.
  import { app, focusedSessionId, machineOfProject } from "../lib/state/app.svelte";
  import { SHORTCUT_LABELS } from "../lib/input/shortcuts";

  const projectId = $derived.by(() => {
    const focused = focusedSessionId();
    return (focused ? app.model.sessions[focused]?.project_id : undefined) ?? app.selectedProjectId;
  });
  const project = $derived(projectId ? app.model.projects[projectId] : undefined);
  const machine = $derived(project ? app.machines.find((m) => m.id === machineOfProject(project.id)) : undefined);
</script>

<footer class="status">
  <span>{machine?.label ?? "This PC"}</span>
  {#if project}<span class="path">{project.path}</span>{/if}
  <span class="grow"></span>
  <span>{SHORTCUT_LABELS.switcher} to switch</span>
</footer>

<style>
  .path {
    font-family: var(--jojo-font-mono);
  }
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
