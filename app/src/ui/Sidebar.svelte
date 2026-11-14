<script lang="ts">
  import { addProject, app, displayLabel, focusPane, focusedSessionId, isFolded, newSession, projectsInOrder, setGroupsFolded, removeMachine, removeProject, renameProject, renameSession, reorderProject, restartDaemon, selectProject, upgradeMachine } from "../lib/state/app.svelte";
  import { sessionMenu } from "../lib/state/contextMenus";
  import { formatElapsed } from "../lib/state/elapsed";
  import { beginRename, endRename, openDialog, openMenu, ui } from "../lib/state/ui.svelte";
  import type { Machine } from "../lib/backend/Machine";
  import type { MenuEntry } from "../lib/menu/types";
  import { shellChoices } from "../lib/state/shells";
  import { sessionsForProject } from "../lib/state/views";
  import { contextMeter, contextTooltip } from "../lib/state/context";
  import { machineStatusText } from "../lib/state/machines";
  import { stateTooltip } from "../lib/state/toasts";
  import { stateToken } from "../lib/state/attention";
  import { foldedStates } from "../lib/state/folding";
  import type { SessionInfo } from "../lib/protocol/SessionInfo";
  import Badge from "./Badge.svelte";
  import Cog from "./Cog.svelte";
  import NeedsYouQueue from "./NeedsYouQueue.svelte";
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
        { kind: "action", label: "Remove remote location…", run: () => void removeMachine(machine.id, false) },
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
      { kind: "action", label: "Rename project", run: () => beginRename({ kind: "project", projectId }) },
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
    if (event.button !== 0 || (event.target as HTMLElement).closest(".add, .chev-button")) return;
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
  /** Left folds and Right unfolds the location or project whose row has keyboard focus. */
  function onFoldKey(event: KeyboardEvent, ids: string[]): void {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    setGroupsFolded(ids, event.key === "ArrowLeft");
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

<!-- The fold chevron: points down when unfolded, right when folded. -->
{#snippet chevron(folded: boolean)}
  <svg class="chev" class:closed={folded} viewBox="0 0 10 10" width="9" height="9" aria-hidden="true">
    <path d="M3.2 1.8 6.6 5 3.2 8.2" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
  </svg>
{/snippet}

<!-- A folded row's summary of its hidden sessions: one dot per session
     with a state color (folding.ts foldedStates); needs you pulses. -->
{#snippet pips(sessions: SessionInfo[])}
  {#each foldedStates(sessions) as state, i (i)}
    <span class="pip" class:needs={state === "needs_you"} style:--pip={`var(${stateToken(state)})`}></span>
  {/each}
{/snippet}

<aside class="side">
  <!-- The JOJO logo (Alexandria SemiBold in the brand gradient). Dragging the
       row moves the window; on macOS the traffic-light buttons sit at its
       left (trafficLightPosition in tauri.macos.conf.json). -->
  <div class="brand" data-tauri-drag-region><span class="logo" data-tauri-drag-region>JOJO</span></div>
  <NeedsYouQueue />
  {#each app.machines as machine (machine.id)}
  {@const status = app.statuses[machine.id]}
  {@const statusText = machineStatusText(machine, status)}
  {@const machineFolded = isFolded(machine.id)}
  <div class="machine-group" class:offline={status?.state !== "connected"} class:folded={machineFolded}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="machine fold-row" title={statusText ?? machine.label} oncontextmenu={(e) => onMachineMenu(e, machine)}>
    <button
      class="chev-button"
      aria-label={`${machineFolded ? "Unfold" : "Fold"} ${machine.label}`}
      aria-expanded={!machineFolded}
      title={machineFolded ? "Unfold" : "Fold"}
      onclick={() => setGroupsFolded([machine.id], !machineFolded)}
    >{@render chevron(machineFolded)}</button>
    <span class="dot" class:ok={status?.state === "connected"}></span>
    <button
      class="grow fold-label"
      aria-expanded={!machineFolded}
      onclick={() => setGroupsFolded([machine.id], !machineFolded)}
      onkeydown={(e) => onFoldKey(e, [machine.id])}
    >{machine.label}{#if statusText}<span class="state"> · {statusText}</span>{/if}</button>
    {#if machineFolded}
      {@const projects = projectsOf(machine.id)}
      <span class="summary">{@render pips(projects.flatMap((p) => sessionsForProject(app.model, p.id)))}<span class="n">{projects.length} {projects.length === 1 ? "project" : "projects"}</span></span>
    {/if}
    {#if machine.kind === "local"}
      <button class="icon" title="Add project" aria-label="Add project" onclick={addProject}><PlusCircle /></button>
    {:else}
      <button class="icon" title="Add project" aria-label={`Add project on ${machine.label}`} onclick={() => openDialog({ kind: "add-project", machineId: machine.id })}><PlusCircle /></button>
    {/if}
  </div>
  <div class="kids" class:folded={machineFolded} inert={machineFolded}><div>
  {#each projectsOf(machine.id) as project, index (project.id)}
    {@const projectSessions = sessionsForProject(app.model, project.id)}
    {@const count = projectSessions.length}
    {@const projectFolded = isFolded(project.id)}
    {@const dropHere = dragging !== null && dragging.machineId === machine.id}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="project fold-row"
      class:folded={projectFolded}
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
      <!-- Alt+click folds or unfolds every project of this location. -->
      <button
        class="chev-button"
        aria-label={`${projectFolded ? "Unfold" : "Fold"} ${project.name}`}
        aria-expanded={!projectFolded}
        title={`${projectFolded ? "Unfold" : "Fold"} (Alt+click: every project on ${machine.label})`}
        onclick={(e) => setGroupsFolded(e.altKey ? projectsOf(machine.id).map((p) => p.id) : [project.id], !projectFolded)}
      >{@render chevron(projectFolded)}</button>
      {#if ui.renaming?.kind === "project" && ui.renaming.projectId === project.id}
        <RenameField
          value={project.name}
          label="Project name"
          onCommit={(value) => {
            endRename();
            renameProject(project.id, value);
          }}
          onCancel={endRename}
        />
      {:else}
        <button
          class="name"
          title={project.path}
          aria-expanded={!projectFolded}
          onclick={() => selectProject(project.id)}
          ondblclick={() => beginRename({ kind: "project", projectId: project.id })}
          onkeydown={(e) => onFoldKey(e, [project.id])}
        >
          {project.name}{#if projectFolded}<span class="summary">{@render pips(projectSessions)}</span>{:else if count > 0}<small class="count">{count}</small>{/if}
        </button>
      {/if}
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
    <div class="kids" class:folded={projectFolded} inert={projectFolded}><div>
    {#each projectSessions as session (session.id)}
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
          {#if session.context_tokens !== null}
            {@const meter = contextMeter(session.context_tokens, session.context_window)}
            {#if meter}
              <span class="ctx {meter.level}" title={contextTooltip(meter)} aria-label={contextTooltip(meter)}>
                <span class="fill" style:width={`${Math.max(4, meter.fraction * 100)}%`}></span>
              </span>
            {/if}
          {/if}
          <span class="time">{formatElapsed(app.now - session.state_changed_at)}</span>
        </button>
      {/if}
    {/each}
    </div></div>
  {/each}
  </div></div>
  </div>
  {/each}
  <div class="footer">
    <button class="add-machine" onclick={() => openDialog({ kind: "add-machine" })}><PlusCircle /><span>Remote location</span></button>
    <button class="settings" title="Settings" aria-label="Settings" onclick={() => openDialog({ kind: "settings" })}><Cog size={22} /></button>
  </div>
</aside>

<style>
  /* Design pass 1.2: a floating frosted panel. */
  .side {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin: var(--jojo-side-margin);
    padding: 10px 8px;
    background: var(--jojo-glass-side);
    backdrop-filter: blur(var(--jojo-side-blur));
    border: var(--jojo-bw) solid var(--jojo-line);
    border-radius: var(--jojo-side-radius);
    box-shadow: var(--jojo-panel-shadow);
    overflow-y: auto;
    min-height: 0;
  }
  .brand {
    flex: none;
    padding: 2px 6px 10px;
  }
  /* macOS: room for the traffic-light buttons left of the logo. */
  :global(html[data-platform="mac"]) .brand {
    padding-left: 72px;
  }
  .logo {
    display: inline-block;
    font-family: "Alexandria", var(--jojo-font-ui);
    font-size: 26px;
    font-weight: 600;
    line-height: 1;
    letter-spacing: 0.04em;
    background: var(--jojo-brand-gradient);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
    user-select: none;
  }
  /* A machine's name heads its projects: 13 px semibold in the main text
     color, above the 12.5 px project names, so each machine's block reads
     as one group. */
  .machine {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 8px 4px 22px;
    font-size: 13px;
    font-weight: 600;
    color: var(--jojo-fg);
  }
  /* A line and space between one machine's block and the next. */
  .machine-group + .machine-group {
    margin-top: 8px;
    padding-top: 4px;
    border-top: var(--jojo-bw) solid var(--jojo-line);
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
    border-top: var(--jojo-bw) solid var(--jojo-line);
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
    font-size: 11.5px;
    font-weight: 400;
    color: var(--jojo-muted);
  }
  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: var(--jojo-dot-radius);
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
    padding: 6px 8px 2px 22px;
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
  /* The context meter: how full the Claude conversation's context window
     is (lib/state/context.ts). Accent while there is room, amber from 70%,
     red from 85%. */
  .ctx {
    flex: none;
    width: 30px;
    height: 5px;
    border-radius: 3px;
    background: var(--jojo-line);
    overflow: hidden;
  }
  .ctx .fill {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: var(--jojo-accent);
  }
  .ctx.warn .fill {
    background: var(--jojo-needs-you);
  }
  .ctx.full .fill {
    background: var(--jojo-error);
  }
  .session {
    display: flex;
    width: 100%;
    align-items: center;
    gap: 8px;
    padding: 5px 8px 5px 28px;
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
  /* Folding (lib/state/folding.ts, layout.json's collapsed). A 9 px chevron
     button sits in the 22 px left padding of each location and project
     row, inside the row (the folding containers clip anything outside
     it). It is hidden until the row is hovered or focused, and stays
     faintly visible, pointing right, while the group is folded. */
  .fold-row {
    position: relative;
  }
  .chev {
    flex: none;
    color: var(--jojo-muted);
    opacity: 0;
    transform: rotate(90deg);
    transition:
      opacity 120ms,
      transform 140ms ease;
  }
  .chev-button {
    position: absolute;
    left: 4px;
    top: 0;
    bottom: 0;
    width: 16px;
    /* Both rows have 4 px more padding above their text than below (8/4
       and 6/2); 4 px here centers the chevron on the text. */
    padding-top: 4px;
    display: grid;
    place-items: center;
  }
  .fold-row:hover .chev,
  .fold-row:focus-within .chev {
    opacity: 0.75;
  }
  .chev.closed {
    opacity: 0.45;
    transform: none;
  }
  .fold-row:hover .chev.closed {
    opacity: 0.85;
  }
  .fold-label {
    min-width: 0;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* A group's rows fold with a 160 ms height transition (grid 1fr to 0fr). */
  .kids {
    display: grid;
    grid-template-rows: 1fr;
    transition: grid-template-rows 160ms ease;
  }
  .kids > div {
    min-height: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .kids.folded {
    grid-template-rows: 0fr;
  }
  /* A folded row's summary: a 5 px dot per session with a state color, and
     on a location its project count. */
  .summary {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    margin-left: 6px;
    vertical-align: middle;
  }
  .pip {
    width: 5px;
    height: 5px;
    border-radius: var(--jojo-dot-radius);
    background: var(--pip);
  }
  .pip.needs {
    box-shadow: 0 0 6px var(--jojo-needs-you-glow);
    animation: pip-pulse 1.6s ease-in-out infinite;
  }
  @keyframes pip-pulse {
    50% {
      opacity: 0.45;
    }
  }
  .summary .n {
    margin-left: 3px;
    color: var(--jojo-muted);
    font-size: 10.5px;
    font-weight: 400;
  }
  @media (prefers-reduced-motion: reduce) {
    .kids,
    .chev {
      transition: none;
    }
    .pip.needs {
      animation: none;
    }
  }
</style>
