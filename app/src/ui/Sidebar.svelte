<script lang="ts">
  import { addProject, app, focusPane, focusedSessionId, newSession, removeMachine, renameSession, restartDaemon, selectProject, upgradeMachine } from "../lib/state/app.svelte";
  import { sessionMenu } from "../lib/state/contextMenus";
  import { formatElapsed } from "../lib/state/elapsed";
  import { beginRename, endRename, openDialog, openMenu, ui } from "../lib/state/ui.svelte";
  import type { Machine } from "../lib/backend/Machine";
  import type { MenuEntry } from "../lib/menu/types";
  import { shellChoices } from "../lib/state/shells";
  import { sessionsForProject, sortedProjects } from "../lib/state/views";
  import { emptyModel } from "../lib/state/reducer";
  import { machineStatusText } from "../lib/state/machines";
  import { stateTooltip } from "../lib/state/toasts";
  import Badge from "./Badge.svelte";
  import RenameField from "./RenameField.svelte";

  /** Right-click on a machine header (spec 8.4): Reconnect, and Remove for remote machines. */
  function onMachineMenu(event: MouseEvent, machine: Machine): void {
    event.preventDefault();
    const entries: MenuEntry[] = [
      { kind: "action", label: machine.kind === "local" ? "Restart jojod" : "Reconnect", run: () => restartDaemon(machine.id) },
    ];
    if (machine.kind === "ssh" && app.statuses[machine.id]?.state === "version_mismatch") {
      entries.unshift({ kind: "action", label: "Upgrade jojod…", run: () => void upgradeMachine(machine.id) });
    }
    if (machine.kind === "ssh") {
      entries.push(
        { kind: "action", label: "Add project…", run: () => openDialog({ kind: "add-project", machineId: machine.id }) },
        { kind: "separator" },
        { kind: "action", label: "Remove machine…", run: () => void removeMachine(machine.id, false) },
        { kind: "action", label: "Remove and uninstall from server…", run: () => void removeMachine(machine.id, true) },
      );
    }
    openMenu(event.clientX, event.clientY, entries);
  }

  /** A machine's projects, sorted by name; empty until its first Projects reply. */
  const projectsOf = (machineId: string) => sortedProjects(app.models[machineId] ?? emptyModel());
  const shells = $derived(shellChoices(app.shells, app.settings?.defaultShell));

  function isRenaming(sessionId: string): boolean {
    return ui.renaming?.kind === "session" && ui.renaming.where === "sidebar" && ui.renaming.sessionId === sessionId;
  }

  function onSessionMenu(event: MouseEvent, sessionId: string): void {
    event.preventDefault();
    openMenu(event.clientX, event.clientY, sessionMenu(sessionId));
  }

  /**
   * Right-click on a project's +: a terminal (the same as a click), a Claude
   * session, and on This PC the other detected shells. A remote project's
   * terminal is the server's login shell.
   */
  function onNewSessionMenu(event: MouseEvent, projectId: string, isLocal: boolean): void {
    event.preventDefault();
    const entries: MenuEntry[] = [
      { kind: "action", label: "New terminal", shortcut: "Ctrl+Shift+N", run: () => newSession(projectId, "shell") },
      { kind: "action", label: "New Claude session", shortcut: "Ctrl+Shift+T", run: () => newSession(projectId, "claude") },
    ];
    const others = isLocal ? shells.filter((shell) => !shell.isDefault) : [];
    if (others.length > 0) {
      entries.push({ kind: "separator" });
      for (const shell of others) {
        entries.push({ kind: "action", label: `New ${shell.label}`, run: () => newSession(projectId, "shell", { kind: "tab" }, shell.program) });
      }
    }
    openMenu(event.clientX, event.clientY, entries);
  }
</script>

<aside class="side">
  <div class="brand" data-tauri-drag-region>jojo</div>
  {#each app.machines as machine (machine.id)}
  {@const status = app.statuses[machine.id]}
  {@const statusText = machineStatusText(machine, status)}
  <div class="machine-group" class:offline={status?.state !== "connected"}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="machine" title={statusText ?? machine.label} oncontextmenu={(e) => onMachineMenu(e, machine)}>
    <span class="dot" class:ok={status?.state === "connected"}></span>
    <span class="grow">{machine.label}{#if statusText}<span class="state"> · {statusText}</span>{/if}</span>
    {#if machine.kind === "local"}
      <button class="icon" title="Add project" aria-label="Add project" onclick={addProject}>+</button>
    {:else}
      <button class="icon" title="Add project" aria-label={`Add project on ${machine.label}`} onclick={() => openDialog({ kind: "add-project", machineId: machine.id })}>+</button>
    {/if}
  </div>
  {#each projectsOf(machine.id) as project (project.id)}
    {@const count = sessionsForProject(app.model, project.id).length}
    <div class="project" class:selected={app.selectedProjectId === project.id}>
      <button class="name" onclick={() => selectProject(project.id)}>
        {project.name}{#if count > 0}<small class="count">{count}</small>{/if}
      </button>
      <!-- A click opens a terminal in the project (type `claude` in it);
           right-click offers a Claude session and other shells. -->
      <button
        class="icon add"
        title="New terminal (right-click for more)"
        aria-label={`New terminal in ${project.name}`}
        onclick={() => newSession(project.id, "shell")}
        oncontextmenu={(e) => onNewSessionMenu(e, project.id, machine.kind === "local")}
      >+</button>
    </div>
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
  </div>
  {/each}
  <div class="footer">
    <button class="add-machine" onclick={() => openDialog({ kind: "add-machine" })}>+ Machine</button>
    <button class="settings" title="Settings" aria-label="Settings" onclick={() => openDialog({ kind: "settings" })}>⚙</button>
  </div>
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
  /* Spec 8.3 rule 1: a machine that is not connected keeps its sessions
     and their last badges, dimmed. */
  .count {
    margin-left: 6px;
    color: var(--jojo-muted);
    font-size: 10.5px;
    font-weight: 400;
  }
  .footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: auto;
    padding: 8px;
    border-top: 1px solid var(--jojo-line);
  }
  .add-machine,
  .settings {
    color: var(--jojo-muted);
  }
  .settings:hover,
  .add-machine:hover {
    color: var(--jojo-fg);
  }
  .machine-group.offline {
    opacity: 0.55;
  }
  .state {
    text-transform: none;
    letter-spacing: 0;
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
