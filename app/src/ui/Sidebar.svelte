<script lang="ts">
  import { activateSession, addProject, app, newSession, selectProject } from "../lib/state/app.svelte";
  import { formatElapsed } from "../lib/state/elapsed";
  import { sessionsForProject, sortedProjects } from "../lib/state/views";

  let menuFor = $state<string | null>(null);
  const projects = $derived(sortedProjects(app.model));
</script>

<aside class="side">
  <div class="brand" data-tauri-drag-region>jojo</div>
  <div class="machine">
    <span class="dot" class:ok={app.status.state === "connected"}></span>
    <span class="grow">This PC</span>
    <button class="icon" title="Add project" onclick={addProject}>+</button>
  </div>
  {#each projects as project (project.id)}
    <div class="project" class:selected={app.selectedProjectId === project.id}>
      <button class="name" onclick={() => selectProject(project.id)}>{project.name}</button>
      <button class="icon add" title="New session" onclick={() => (menuFor = menuFor === project.id ? null : project.id)}>+</button>
    </div>
    {#if menuFor === project.id}
      <div class="menu">
        <button onclick={() => { menuFor = null; newSession(project.id, "claude"); }}>Claude</button>
        <button onclick={() => { menuFor = null; newSession(project.id, "shell"); }}>Shell</button>
      </div>
    {/if}
    {#each sessionsForProject(app.model, project.id) as session (session.id)}
      <button class="session" class:active={app.activeSessionId === session.id} onclick={() => activateSession(session)}>
        <span class="badge {session.state}"></span>
        <span class="label">{session.label}</span>
        <span class="time">{formatElapsed(app.now - session.state_changed_at)}</span>
      </button>
    {/each}
  {/each}
</aside>

<style>
  .side {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 10px 8px;
    background: var(--jojo-side);
    border-right: 1px solid var(--jojo-line);
    overflow-y: auto;
    min-height: 0;
  }
  .brand {
    padding: 4px 8px 10px;
    font-size: 15px;
    font-weight: 600;
    background: var(--jojo-brand-gradient);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }
  .machine {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 10px 8px 4px;
    font-size: 10.5px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--jojo-muted);
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--jojo-needs-you);
  }
  .dot.ok {
    background: var(--jojo-done);
  }
  .grow {
    flex: 1;
  }
  .icon {
    color: var(--jojo-muted);
    padding: 0 4px;
  }
  .icon:hover {
    color: var(--jojo-fg);
  }
  .project {
    display: flex;
    align-items: center;
    padding: 6px 8px 2px;
  }
  .project .name {
    flex: 1;
    text-align: left;
    color: var(--jojo-fg2);
    font-weight: 500;
  }
  .project.selected .name {
    color: var(--jojo-fg);
  }
  .project .add {
    visibility: hidden;
  }
  .project:hover .add {
    visibility: visible;
  }
  .menu {
    display: flex;
    gap: 4px;
    padding: 2px 8px 4px 14px;
  }
  .menu button {
    padding: 3px 8px;
    border: 1px solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
    color: var(--jojo-fg2);
  }
  .menu button:hover {
    background: var(--jojo-sel);
  }
  .session {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px 5px 14px;
    border-radius: var(--jojo-radius);
    color: var(--jojo-fg2);
    text-align: left;
  }
  .session.active {
    background: var(--jojo-sel);
    color: var(--jojo-fg);
    box-shadow: inset 2px 0 0 var(--jojo-accent);
  }
  .label {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .time {
    color: var(--jojo-muted);
    font-size: 10.5px;
  }
  .badge {
    width: 13px;
    height: 13px;
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
