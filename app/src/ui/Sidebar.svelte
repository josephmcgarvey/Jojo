<script lang="ts">
  import { addProject, app, focusPane, focusedSessionId, newSession, renameSession, selectProject } from "../lib/state/app.svelte";
  import { sessionMenu } from "../lib/state/contextMenus";
  import { formatElapsed } from "../lib/state/elapsed";
  import { beginRename, endRename, openMenu, ui } from "../lib/state/ui.svelte";
  import { shellChoices } from "../lib/state/shells";
  import { sessionsForProject, sortedProjects } from "../lib/state/views";
  import { stateTooltip } from "../lib/state/toasts";
  import Badge from "./Badge.svelte";
  import RenameField from "./RenameField.svelte";

  let menuFor = $state<string | null>(null);
  const projects = $derived(sortedProjects(app.model));
  const shells = $derived(shellChoices(app.shells, app.settings?.defaultShell));

  function isInsideOpenMenu(target: EventTarget | null): boolean {
    return target instanceof Element && target.closest(`[data-session-menu="${menuFor}"]`) !== null;
  }

  function isRenaming(sessionId: string): boolean {
    return ui.renaming?.kind === "session" && ui.renaming.where === "sidebar" && ui.renaming.sessionId === sessionId;
  }

  function onSessionMenu(event: MouseEvent, sessionId: string): void {
    event.preventDefault();
    openMenu(event.clientX, event.clientY, sessionMenu(sessionId));
  }

  // The Claude/Shell menu is a single piece of state (menuFor), so only one
  // project's menu is ever open; these listeners are only attached while
  // that is true, and torn down as soon as it closes.
  $effect(() => {
    if (menuFor === null) return;
    const onKeydown = (e: KeyboardEvent) => {
      if (e.key === "Escape") menuFor = null;
    };
    const onClick = (e: MouseEvent) => {
      // Reads menuFor live, not a stale closed-over value: when this click
      // is the same one that toggled the menu via the "+" button's own
      // onclick, that handler already ran (target-phase listeners fire
      // before this bubble-phase one), so menuFor already reflects the
      // toggle's result and isInsideOpenMenu is checked against it.
      if (!isInsideOpenMenu(e.target)) menuFor = null;
    };
    window.addEventListener("keydown", onKeydown);
    window.addEventListener("click", onClick);
    return () => {
      window.removeEventListener("keydown", onKeydown);
      window.removeEventListener("click", onClick);
    };
  });
</script>

<aside class="side">
  <div class="brand" data-tauri-drag-region>jojo</div>
  <div class="machine">
    <span class="dot" class:ok={app.status.state === "connected"}></span>
    <span class="grow">This PC</span>
    <button class="icon" title="Add project" aria-label="Add project" onclick={addProject}>+</button>
  </div>
  {#each projects as project (project.id)}
    <div class="project" class:selected={app.selectedProjectId === project.id}>
      <button class="name" onclick={() => selectProject(project.id)}>{project.name}</button>
      <button
        class="icon add"
        title="New session"
        aria-label={`New session in ${project.name}`}
        data-session-menu={project.id}
        onclick={() => (menuFor = menuFor === project.id ? null : project.id)}
      >+</button>
    </div>
    {#if menuFor === project.id}
      <div class="menu" data-session-menu={project.id}>
        <button onclick={() => { menuFor = null; newSession(project.id, "claude"); }}>Claude</button>
        {#each shells as shell (shell.program)}
          <button onclick={() => { menuFor = null; newSession(project.id, "shell", { kind: "tab" }, shell.program); }}>{shell.label}</button>
        {:else}
          <button onclick={() => { menuFor = null; newSession(project.id, "shell"); }}>Shell</button>
        {/each}
      </div>
    {/if}
    {#each sessionsForProject(app.model, project.id) as session (session.id)}
      {#if isRenaming(session.id)}
        <div class="session renaming">
          <Badge agentState={session.state} title={stateTooltip(session)} />
          <RenameField
            value={session.label}
            label="Session name"
            onCommit={(value) => {
              endRename();
              renameSession(session.id, value);
            }}
            onCancel={endRename}
          />
        </div>
      {:else}
        <button
          class="session"
          class:active={focusedSessionId() === session.id}
          onclick={() => focusPane(session.id)}
          ondblclick={() => beginRename({ kind: "session", sessionId: session.id, where: "sidebar" })}
          oncontextmenu={(e) => onSessionMenu(e, session.id)}
        >
          <Badge agentState={session.state} title={stateTooltip(session)} />
          <span class="label">{session.label}</span>
          <span class="time">{formatElapsed(app.now - session.state_changed_at)}</span>
        </button>
      {/if}
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
    opacity: 0;
  }
  .project:hover .add,
  .project:focus-within .add {
    opacity: 1;
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
</style>
