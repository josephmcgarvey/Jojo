<script lang="ts">
  // Spec 9.7: the focused session's machine label and project path (the
  // selected project's when no pane is focused), and the switcher hint.
  import { app, focusedSessionId, machineOfProject } from "../lib/state/app.svelte";
  import { SHORTCUT_LABELS } from "../lib/input/shortcuts";
  import { LOCAL_LABEL } from "../lib/platform";

  const projectId = $derived.by(() => {
    const focused = focusedSessionId();
    return (focused ? app.model.sessions[focused]?.project_id : undefined) ?? app.selectedProjectId;
  });
  const project = $derived(projectId ? app.model.projects[projectId] : undefined);
  const machine = $derived(project ? app.machines.find((m) => m.id === machineOfProject(project.id)) : undefined);
</script>

<footer class="status">
  <span class="pill">{machine?.label ?? LOCAL_LABEL}</span>
  {#if project}<span class="pill path">{project.path}</span>{/if}
  <span class="grow"></span>
  <span><kbd>{SHORTCUT_LABELS.switcher}</kbd> to switch</span>
</footer>

<style>
  .path {
    font-family: var(--jojo-font-mono);
  }
  /* Design pass 2.6: pills over the window glow. */
  .status {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 2px 12px 8px;
    font-size: 11px;
    color: var(--jojo-muted);
  }
  .pill {
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--jojo-pill);
    border: 1px solid var(--jojo-line);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  kbd {
    padding: 1px 5px;
    border-radius: 4px;
    border: 1px solid var(--jojo-line);
    color: var(--jojo-fg2);
    font-family: var(--jojo-font-mono);
    font-size: 10px;
  }
  .grow {
    flex: 1;
  }
</style>
