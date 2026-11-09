<script lang="ts">
  import { addProject, app, displayLabel, focusPane, focusedSessionId, newSession, projectsInOrder, removeMachine, removeProject, renameSession, reorderProject, restartDaemon, selectProject, upgradeMachine } from "../lib/state/app.svelte";
  import { sessionMenu } from "../lib/state/contextMenus";
  import { formatElapsed } from "../lib/state/elapsed";
  import { beginRename, endRename, openDialog, openMenu, ui } from "../lib/state/ui.svelte";
  import type { Machine } from "../lib/backend/Machine";
  import type { MenuEntry } from "../lib/menu/types";
  import { shellChoices } from "../lib/state/shells";
  import { sessionsForProject } from "../lib/state/views";
  import { machineStatusText } from "../lib/state/machines";
  import { stateTooltip } from "../lib/state/toasts";
  import Badge from "./Badge.svelte";
  import { isMac } from "../lib/platform";
  import Cog from "./Cog.svelte";
  import PlusCircle from "./PlusCircle.svelte";
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

  /** A machine's projects in sidebar order (dragged order, then by name); empty until its first Projects reply. */
  const projectsOf = (machineId: string) => projectsInOrder(machineId);

  /** Right-click on a project's name: new sessions and Remove project. */
  function onProjectMenu(event: MouseEvent, projectId: string): void {
    event.preventDefault();
    openMenu(event.clientX, event.clientY, [
      { kind: "action", label: "New terminal", run: () => newSession(projectId, "shell") },
      { kind: "action", label: "New Claude session", run: () => newSession(projectId, "claude") },
      { kind: "separator" },
      { kind: "action", label: "Remove project…", run: () => void removeProject(projectId) },
    ]);
  }

  /**
   * Drag a project's name to reorder the projects of its machine, the same
   * way tabs are dragged (TabBar.svelte): more than DRAG_THRESHOLD pixels
   * starts a drag, and the drop position is the number of the machine's
   * project rows whose vertical center is above the pointer. A project's
   * sessions move with it. Projects do not move between machines.
   */
  const DRAG_THRESHOLD = 5;
  let press: { machineId: string; projectId: string; startY: number; pointerId: number; element: HTMLElement } | null = null;
  let dragging = $state<{ machineId: string; projectId: string; dropIndex: number } | null>(null);

  function projectDropIndex(machineId: string, y: number): number {
    const rows = [...document.querySelectorAll<HTMLElement>(`[data-project-machine="${CSS.escape(machineId)}"]`)];
    return rows.filter((el) => {
      const r = el.getBoundingClientRect();
      return r.top + r.height / 2 < y;
    }).length;
  }

  function onProjectPointerDown(event: PointerEvent, machineId: string, projectId: string): void {
    if (event.button !== 0 || (event.target as HTMLElement).closest(".add")) return;
    press = { machineId, projectId, startY: event.clientY, pointerId: event.pointerId, element: event.currentTarget as HTMLElement };
  }

  function onProjectPointerMove(event: PointerEvent): void {
    if (!press || event.pointerId !== press.pointerId) return;
    if (!dragging) {
      if (Math.abs(event.clientY - press.startY) < DRAG_THRESHOLD) return;
      press.element.setPointerCapture(event.pointerId);
    }
    dragging = { machineId: press.machineId, projectId: press.projectId, dropIndex: projectDropIndex(press.machineId, event.clientY) };
  }

  function onProjectPointerUp(event: PointerEvent): void {
    if (!press || event.pointerId !== press.pointerId) return;
    if (dragging) {
      reorderProject(dragging.machineId, dragging.projectId, dragging.dropIndex);
      // As in TabBar: swallow only the click that follows this drag.
      const swallow = (e: MouseEvent) => e.stopPropagation();
      window.addEventListener("click", swallow, { capture: true, once: true });
      setTimeout(() => window.removeEventListener("click", swallow, { capture: true }), 0);
    }
    press = null;
    dragging = null;
  }

  function onProjectPointerCancel(): void {
    press = null;
    dragging = null;
  }
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
  <!-- macOS: the traffic-light buttons sit in this strip (trafficLightPosition
       in tauri.macos.conf.json), and dragging it moves the window. -->
  {#if isMac}<div class="traffic-lights" data-tauri-drag-region></div>{/if}
  {#each app.machines as machine (machine.id)}
  {@const status = app.statuses[machine.id]}
  {@const statusText = machineStatusText(machine, status)}
  <div class="machine-group" class:offline={status?.state !== "connected"}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="machine" title={statusText ?? machine.label} oncontextmenu={(e) => onMachineMenu(e, machine)}>
    <span class="dot" class:ok={status?.state === "connected"}></span>
    <span class="grow">{machine.label}{#if statusText}<span class="state"> · {statusText}</span>{/if}</span>
    {#if machine.kind === "local"}
      <button class="icon" title="Add project" aria-label="Add project" onclick={addProject}><PlusCircle /></button>
    {:else}
      <button class="icon" title="Add project" aria-label={`Add project on ${machine.label}`} onclick={() => openDialog({ kind: "add-project", machineId: machine.id })}><PlusCircle /></button>
    {/if}
  </div>
  {#each projectsOf(machine.id) as project, index (project.id)}
    {@const count = sessionsForProject(app.model, project.id).length}
    {@const dropHere = dragging !== null && dragging.machineId === machine.id}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="project"
      class:selected={app.selectedProjectId === project.id}
      class:dragging={dragging?.projectId === project.id}
      class:drop-before={dropHere && dragging?.dropIndex === index}
      class:drop-after={dropHere && index === projectsOf(machine.id).length - 1 && dragging?.dropIndex === index + 1}
      data-project-machine={machine.id}
      oncontextmenu={(e) => onProjectMenu(e, project.id)}
      onpointerdown={(e) => onProjectPointerDown(e, machine.id, project.id)}
      onpointermove={onProjectPointerMove}
      onpointerup={onProjectPointerUp}
      onpointercancel={onProjectPointerCancel}
    >
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
      ><PlusCircle /></button>
    </div>
    {#each sessionsForProject(app.model, project.id) as session (session.id)}
      {#if isRenaming(session.id)}
        <div class="session renaming">
          <Badge agentState={session.state} title={stateTooltip(session)} />
          <RenameField
            value={displayLabel(session)}
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
          <span class="label">{displayLabel(session)}</span>
          <span class="time">{formatElapsed(app.now - session.state_changed_at)}</span>
        </button>
      {/if}
    {/each}
  {/each}
  </div>
  {/each}
  <div class="footer">
    <button class="add-machine" onclick={() => openDialog({ kind: "add-machine" })}><PlusCircle /><span>Machine</span></button>
    <button class="settings" title="Settings" aria-label="Settings" onclick={() => openDialog({ kind: "settings" })}><Cog size={22} /></button>
  </div>
</aside>

<style>
  /* Design pass 1.2: a floating frosted panel. */
  .side {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin: 8px 0 8px 8px;
    padding: 10px 8px;
    background: var(--jojo-glass-side);
    backdrop-filter: blur(16px);
    border: 1px solid var(--jojo-line);
    border-radius: var(--jojo-radius);
    overflow-y: auto;
    min-height: 0;
  }
  .traffic-lights {
    flex: none;
    height: 22px;
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
    display: flex;
    align-items: center;
    color: var(--jojo-muted);
  }
  .add-machine {
    gap: 6px;
    padding: 4px 6px 4px 2px;
    font-weight: 500;
  }
  .settings {
    padding: 3px;
    border-radius: var(--jojo-radius-sm);
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
  /* Design pass 2.2: the filled + icons. */
  .icon {
    display: flex;
    align-items: center;
    padding: 2px;
    border-radius: 50%;
    color: var(--jojo-muted);
  }
  .icon:hover {
    color: var(--jojo-fg);
  }
  .icon:active {
    color: var(--jojo-accent);
  }
  .project {
    display: flex;
    align-items: center;
    padding: 6px 8px 2px;
  }
  .project.dragging {
    opacity: 0.55;
  }
  /* The drop position: an accent line above the project the dragged one
     will sit before, or below the machine's last project. */
  .project.drop-before {
    box-shadow: inset 0 2px 0 var(--jojo-accent);
  }
  .project.drop-after {
    box-shadow: inset 0 -2px 0 var(--jojo-accent);
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
    width: 100%;
    align-items: center;
    gap: 8px;
    padding: 5px 8px 5px 14px;
    border-radius: var(--jojo-radius);
    color: var(--jojo-fg2);
    text-align: left;
  }
  .session.active {
    background: var(--jojo-sel-gradient);
    color: var(--jojo-fg);
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
