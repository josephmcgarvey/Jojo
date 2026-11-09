// The app's reactive state and actions. The rules live in pure modules
// (../layout, ./workspace, ./sessionRules); this module holds the current
// values, talks to the backend, and keeps layout.json in sync.
import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { tick } from "svelte";
import type { ConnectionStatus } from "../backend/ConnectionStatus";
import type { LayoutFile } from "../backend/LayoutFile";
import type { Machine } from "../backend/Machine";
import type { ProjectLayout } from "../backend/ProjectLayout";
import type { Settings } from "../backend/Settings";
import type { ShellOption } from "../backend/ShellOption";
import type { TabLayout } from "../backend/TabLayout";
import { errorText } from "../errors";
import { loadTerminalFont, terminalFontStack } from "../fonts";
import { LOCAL_LABEL } from "../platform";
import { focusNeighbor, layoutRects, type FocusDirection } from "../layout/geometry";
import { reconcileMachine } from "../layout/reconcile";
import * as tabs from "../layout/tabs";
import { isFull, paneIds, type LayoutNode, type LayoutPreset, type SplitDirection } from "../layout/tree";
import type { ControlMsg } from "../protocol/ControlMsg";
import type { ProjectInfo } from "../protocol/ProjectInfo";
import type { SessionInfo } from "../protocol/SessionInfo";
import type { SessionKind } from "../protocol/SessionKind";
import { router } from "../terminal/router";
import type { Appearance } from "../theme";
import { LOCAL_MACHINE, sendControl, sendInput, subscribe } from "../wire/bridge";
import { newUlid } from "../wire/ulid";
import { askConfirm } from "./dialog.svelte";
import { allTerminals, terminalControls } from "../terminal/registry";
import { applyControl, clearConnectionErrors, emptyModel, type Model } from "./reducer";
import {
  busyStateText,
  canRestart,
  canResume,
  restorePlan,
  closeTabMessage,
  closeTabNeedsConfirmation,
  closeTabSummary,
  closesOnExit,
  isBusy,
  restartShell,
} from "./sessionRules";
import { toastFor } from "./toasts";
import { chimeFor, playChime } from "./chime";
import { shellProgram } from "./shells";
import { orderedProjects, reorderedProjectOrder, sessionsForProject, sortedProjects } from "./views";
import {
  createFailed,
  emptyWorkspace,
  projectOf,
  requestCreate,
  sessionCreated,
  sessionRemoved,
  withProject,
  type Placement,
  type Workspace,
} from "./workspace";

export const app = $state({
  /** Every machine's projects and sessions merged (ids are ULIDs, unique across machines). */
  model: emptyModel() as Model,
  /** Each machine's own model, as its daemon reported it. */
  models: {} as Record<string, Model>,
  /** This PC's connection status (the banner). */
  status: { state: "starting" } as ConnectionStatus,
  /** Every machine's connection status. */
  statuses: {} as Record<string, ConnectionStatus>,
  /** machines.toml, This PC first. */
  machines: [] as Machine[],
  /** Machines whose first Projects and Sessions replies have arrived. */
  loaded: {} as Record<string, { projects: boolean; sessions: boolean }>,
  /** True once any connection to jojod has succeeded in this app run. */
  everConnected: false,
  settings: null as Settings | null,
  /** The Settings screen's unsaved theme, base color and interface font, shown while it is open. */
  themePreview: null as Appearance | null,
  /** Shells installed on this PC (available_shells), for the New shell menus. */
  shells: [] as ShellOption[],
  selectedProjectId: null as string | null,
  workspace: emptyWorkspace() as Workspace,
  /** True once layout.json has been read; nothing is written before that. */
  layoutLoaded: false,
  /** True once the first Sessions list has been applied; reconciliation waits for it. */
  sessionsLoaded: false,
  /** True once the first Projects reply has been applied. */
  projectsLoaded: false,
  now: Date.now(),
  /** The latest DirEntries reply, for the Add project path suggestions (spec 9.2). */
  dirEntries: null as { machine: string; path: string; entries: string[] } | null,
  /** Whether the Jojo window is the foreground window (spec 7.3 rule 2, 7.4 rule 1). */
  windowFocused: typeof document === "undefined" ? true : document.hasFocus(),
});

/** Which machine each project and session is on, rebuilt with `app.model`. */
let projectMachines: Record<string, string> = {};
let sessionMachines: Record<string, string> = {};

/** The machine a project is on (This PC when unknown). */
export function machineOfProject(projectId: string): string {
  return projectMachines[projectId] ?? LOCAL_MACHINE;
}

/** The machine a session is on (This PC when unknown). */
export function machineOfSession(sessionId: string): string {
  return sessionMachines[sessionId] ?? LOCAL_MACHINE;
}

/** True when `machine` is connected; input and requests to it are possible. */
export function machineConnected(machine: string): boolean {
  return app.statuses[machine]?.state === "connected";
}

/** Rebuilds the merged model and the machine lookups after a machine's model changed. */
function mergeModels(): void {
  const merged = emptyModel();
  const projects: Record<string, string> = {};
  const sessions: Record<string, string> = {};
  for (const [machine, model] of Object.entries(app.models)) {
    Object.assign(merged.projects, model.projects);
    Object.assign(merged.sessions, model.sessions);
    for (const id of Object.keys(model.projects)) projects[id] = machine;
    for (const id of Object.keys(model.sessions)) sessions[id] = machine;
    // The banner shows one error: This PC's, or else a remote machine's.
    if (model.lastError && (machine === LOCAL_MACHINE || !merged.lastError)) merged.lastError = model.lastError;
  }
  projectMachines = projects;
  sessionMachines = sessions;
  app.model = merged;
}

/** Unix ms of each session's last toast (spec 7.4 rule 4). Not rendered, so not reactive. */
const lastToastAt = new Map<string, number>();
/** Unix ms of each session's last chime. */
const lastChimeAt = new Map<string, number>();

/**
 * The name shown for a session: the first prompt Claude Code received when
 * the "Name sessions from Claude's first prompt" setting is on and the user
 * has not renamed the session, otherwise its label.
 */
export function displayLabel(session: Pick<SessionInfo, "label" | "renamed" | "prompt_label">): string {
  return app.settings?.promptLabels && !session.renamed && session.prompt_label ? session.prompt_label : session.label;
}

/** Delay from the last layout change to the layout.json write. */
const SAVE_DELAY_MS = 250;
let saveTimer: ReturnType<typeof setTimeout> | undefined;
let saveChain: Promise<unknown> = Promise.resolve();

function scheduleSave(): void {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    const layout = $state.snapshot(app.workspace.layout) as LayoutFile;
    // Chained so two writes never overlap. A failed write is not retried
    // by itself; the next layout change writes the whole file again.
    saveChain = saveChain.then(() => invoke("save_layout", { layout })).catch(() => {});
  }, SAVE_DELAY_MS);
}

/** Replaces the workspace and schedules a layout.json write when the layout changed. */
function commit(next: Workspace): void {
  const layoutChanged = next.layout !== app.workspace.layout;
  app.workspace = next;
  if (layoutChanged && app.layoutLoaded) scheduleSave();
}

export function startApp(): () => void {
  void invoke<Machine[]>("list_machines")
    .then((machines) => {
      app.machines = machines;
    })
    .catch(() => {
      app.machines = [{ id: LOCAL_MACHINE, label: LOCAL_LABEL, kind: "local", ssh_host: null, enabled: true }];
    });
  void invoke<ShellOption[]>("available_shells")
    .then((shells) => {
      app.shells = shells;
    })
    .catch(() => {
      // The menus then offer a single New shell entry, which uses the
      // defaultShell setting.
    });
  void invoke<Settings>("get_settings")
    .then((s) => {
      app.settings = s;
      // Sessions may have arrived before the settings did.
      for (const machine of Object.keys(app.models)) restoreEndedSessions(machine);
    })
    .catch(() => {
      // Settings stay at their built-in defaults; nothing in the UI depends
      // on this having succeeded.
    });
  // layout.json is read before subscribing, so no SessionCreated can be
  // placed into an empty layout that the file then overwrites.
  void loadLayout().then(connect);
  const tick = setInterval(() => (app.now = Date.now()), 30_000);
  const window = getCurrentWindow();
  const unlistenFocus = window.onFocusChanged(({ payload: focused }) => {
    app.windowFocused = focused;
  });
  // document.hasFocus() at module load can be false while the native window
  // is already in front (the window is shown before the page has focus), and
  // no focus event follows until the user switches away and back.
  void window
    .isFocused()
    .then((focused) => {
      app.windowFocused = focused;
    })
    .catch(() => {});
  // Spec 7.4 rule 3: the backend has already brought the window forward.
  const unlistenToast = listen<string>("toast-activated", ({ payload: sessionId }) => focusPane(sessionId));
  return () => {
    clearInterval(tick);
    void unlistenFocus.then((stop) => stop());
    void unlistenToast.then((stop) => stop());
  };
}

async function loadLayout(): Promise<void> {
  try {
    const layout = await invoke<LayoutFile>("get_layout");
    app.workspace = { ...app.workspace, layout };
  } catch {
    // get_layout returns an empty layout for a missing or unreadable file
    // itself; only a failed IPC call lands here, and the empty layout from
    // emptyWorkspace() stays in place. Reconciliation then opens one tab
    // per session.
  }
  app.layoutLoaded = true;
}

function connect(): void {
  void subscribe({
    onStatus(machine, status) {
      app.statuses[machine] = status;
      if (machine === LOCAL_MACHINE) app.status = status;
      if (status.state === "connected") {
        if (machine === LOCAL_MACHINE) app.everConnected = true;
        app.models[machine] = clearConnectionErrors(app.models[machine] ?? emptyModel());
        mergeModels();
        void sendControl({ type: "ListProjects" }, machine).catch(() => {});
        void sendControl({ type: "ListSessions" }, machine).catch(() => {});
      }
    },
    onFrame(machine, frame) {
      if (frame.kind === "control") handleControl(machine, frame.msg);
      else router.deliver(frame);
    },
  }).catch((err: unknown) => {
    // The bridge itself failed to register (not merely "disconnected", which
    // arrives as a status update instead): there is no connection to retry
    // from in-app, so show it the same way a failed daemon launch would be.
    app.status = { state: "failed", message: errorText(err) };
  });
}

function reconcile(machine: string): void {
  // reconcileMachine drops the layout of every project of `machine` missing
  // from its model, so it must not run before that machine's first Projects
  // reply: ListProjects and ListSessions are separate IPC calls, and
  // Sessions can arrive first.
  const loaded = app.loaded[machine];
  if (!app.layoutLoaded || !loaded?.projects || !loaded.sessions) return;
  const model = app.models[machine] ?? emptyModel();
  commit({ ...app.workspace, layout: reconcileMachine(app.workspace.layout, machine, model, newUlid) });
}

function markLoaded(machine: string, part: "projects" | "sessions"): void {
  app.loaded[machine] = { ...(app.loaded[machine] ?? { projects: false, sessions: false }), [part]: true };
  if (machine === LOCAL_MACHINE) {
    if (part === "projects") app.projectsLoaded = true;
    else app.sessionsLoaded = true;
  }
}

function handleControl(machine: string, msg: ControlMsg): void {
  // Folder listings feed the Add project suggestions and nothing else; a
  // folder that does not exist (a half-typed path) is not an error to show.
  if (msg.type === "DirEntries") {
    app.dirEntries = { machine, path: msg.path, entries: msg.entries };
    return;
  }
  if (msg.type === "Error" && msg.request === "ListDir") {
    app.dirEntries = null;
    return;
  }
  if (msg.type === "Error" && msg.request === "Output") {
    router.fellBehind(msg.session_id);
    return;
  }
  if (msg.type === "Error" && msg.session_id) {
    if (msg.request === "Attach") router.attachFailed(msg.session_id);
    if (msg.request === "CreateSession") commit(createFailed(app.workspace, msg.session_id));
  }

  const previousState = msg.type === "SessionUpdated" ? app.model.sessions[msg.session.id]?.state : undefined;
  app.models[machine] = applyControl(app.models[machine] ?? emptyModel(), msg);
  mergeModels();
  if (msg.type === "SessionUpdated") {
    maybeToast(previousState, msg.session);
    maybeChime(previousState, msg.session);
  }

  switch (msg.type) {
    case "Sessions":
      markLoaded(machine, "sessions");
      reconcile(machine);
      break;
    case "Projects":
      markLoaded(machine, "projects");
      reconcile(machine);
      break;
    case "ProjectRemoved":
      reconcile(machine);
      break;
    case "SessionCreated": {
      // A restored shell gets `claude --resume <id>` typed into it; the PTY
      // holds the input until the shell reads it.
      const typed = typeAfterStart.get(msg.session.id);
      if (typed) {
        typeAfterStart.delete(msg.session.id);
        void sendInput(msg.session.id, new TextEncoder().encode(typed), machine).catch(() => {});
      }
      const result = sessionCreated(app.workspace, msg.session, newUlid, machine);
      commit(result.workspace);
      if (result.requested) app.selectedProjectId = msg.session.project_id;
      if (result.closeSessionId) sendClose(result.closeSessionId);
      break;
    }
    case "SessionExited":
      // Plan 3 amendment A (see closesOnExit): `exit` in a shell closes the
      // pane, as does any exit with code 0. SessionExited is an event, so
      // this applies only to exits observed while connected; sessions
      // already exited in a Sessions reply are left for the user to close.
      if (closesOnExit(app.model.sessions[msg.session_id], msg.exit_code, Date.now())) sendClose(msg.session_id);
      break;
    case "SessionRemoved":
      commit(sessionRemoved(app.workspace, msg.session_id));
      break;
  }

  if (!app.selectedProjectId || !app.model.projects[app.selectedProjectId]) {
    app.selectedProjectId = sortedProjects(app.model)[0]?.id ?? null;
  }

  // Sessions carries every session's current attachable value; reattach only
  // once it has been applied, so a reconnect does not race an Attach against
  // stale attachable state from before the daemon restarted.
  if (msg.type === "Sessions") {
    router.reattachAll((sessionId) => machineOfSession(sessionId) === machine);
    restoreEndedSessions(machine);
  }
}

/** Spec 7.4: shows a toast when a session newly needs attention and the user is not looking at it. */
/** Text to type into a session as soon as its SessionCreated arrives (restored shells). */
const typeAfterStart = new Map<string, string>();
/** Sessions already being restored, so a repeated Sessions reply does not restore them twice. */
const restoring = new Set<string>();

/**
 * Brings back the sessions of `machine` that ended because Windows or jojod
 * restarted (restorePlan), each in its own pane position, when the
 * restoreSessions setting is on. Runs once the machine's sessions are
 * listed and placed in the layout, and the settings are loaded.
 */
function restoreEndedSessions(machine: string): void {
  if (!app.settings || app.settings.restoreSessions === false || !app.layoutLoaded) return;
  const model = app.models[machine];
  if (!model || !app.loaded[machine]?.sessions || !app.loaded[machine]?.projects) return;
  for (const session of Object.values(model.sessions)) {
    if (restoring.has(session.id) || !model.projects[session.project_id]) continue;
    const plan = restorePlan(session);
    if (!plan) continue;
    restoring.add(session.id);
    const id = newSession(session.project_id, plan.kind, { kind: "replace", oldId: session.id }, plan.kind === "shell" ? plan.shell : null, plan.resume);
    if (plan.typeAfterStart) typeAfterStart.set(id, plan.typeAfterStart);
  }
}

/** Plays the chime when a session newly finishes, needs a response or fails. */
function maybeChime(previous: SessionInfo["state"] | undefined, session: SessionInfo): void {
  const now = Date.now();
  const state = chimeFor(previous, session.state, app.settings?.sound ?? true, lastChimeAt.get(session.id), now);
  if (!state) return;
  lastChimeAt.set(session.id, now);
  playChime(state);
}

function maybeToast(previous: SessionInfo["state"] | undefined, session: SessionInfo): void {
  const now = Date.now();
  const toast = toastFor(previous, { ...session, label: displayLabel(session) }, {
    focusedSessionId: focusedSessionId(),
    windowFocused: app.windowFocused,
    enabled: app.settings?.toasts ?? true,
    lastToastAt: lastToastAt.get(session.id),
    now,
    projectName: app.model.projects[session.project_id]?.name,
  });
  if (!toast) return;
  lastToastAt.set(session.id, now);
  void invoke("notify", { sessionId: session.id, title: toast.title, body: toast.body }).catch(() => {
    // No toast (for example, notifications are off in Windows); the badge
    // still shows the state.
  });
}

/**
 * A done session becomes idle when the user types into its pane (user
 * request, replacing spec 7.3 rule 2's "when its pane receives focus": a
 * session that finished while its pane was in front flipped back to idle
 * at once, so its done badge never showed). TerminalPane calls this for
 * each keystroke; only the first one in the done state sends MarkSeen.
 */
export function markSeenOnInput(sessionId: string): void {
  if (app.model.sessions[sessionId]?.state !== "done") return;
  void sendControl({ type: "MarkSeen", session_id: sessionId }, machineOfSession(sessionId)).catch(() => {});
}

export function projectLayout(projectId: string): ProjectLayout {
  return projectOf(app.workspace.layout, projectId, machineOfProject(projectId));
}

export function currentProjectLayout(): ProjectLayout | null {
  return app.selectedProjectId ? projectLayout(app.selectedProjectId) : null;
}

export function currentTab(): TabLayout | null {
  const project = currentProjectLayout();
  return project ? tabs.activeTab(project) : null;
}

export function focusedSessionId(): string | null {
  return currentTab()?.focused ?? null;
}

/** Applies a tab operation to one project; false when the operation was refused (returned null). */
function updateProject(projectId: string, update: (project: ProjectLayout) => ProjectLayout | null): boolean {
  const layout = withProject(app.workspace.layout, projectId, update, machineOfProject(projectId));
  if (!layout) return false;
  if (layout !== app.workspace.layout) commit({ ...app.workspace, layout });
  return true;
}

function updateCurrentProject(update: (project: ProjectLayout) => ProjectLayout | null): boolean {
  return app.selectedProjectId ? updateProject(app.selectedProjectId, update) : false;
}

function projectIdOf(sessionId: string): string | null {
  return app.model.sessions[sessionId]?.project_id ?? null;
}

function updateSessionProject(sessionId: string, update: (project: ProjectLayout) => ProjectLayout | null): boolean {
  const projectId = projectIdOf(sessionId);
  return projectId ? updateProject(projectId, update) : false;
}

export function selectProject(id: string): void {
  app.selectedProjectId = id;
}

export function selectTab(tabId: string): void {
  updateCurrentProject((p) => tabs.selectTab(p, tabId));
}

/** Moves a tab of the current project to a drop position in the tab bar (see tabs.reorderTab). */
export function reorderTab(tabId: string, toIndex: number): void {
  updateCurrentProject((p) => tabs.reorderTab(p, tabId, toIndex));
}

export function cycleTab(delta: 1 | -1): void {
  updateCurrentProject((p) => tabs.cycleTab(p, delta));
}

/** Spec 9.3 rule 5: selects the session's project, activates its tab and focuses its pane. */
export function focusPane(sessionId: string): void {
  const projectId = projectIdOf(sessionId);
  if (!projectId) return;
  app.selectedProjectId = projectId;
  updateProject(projectId, (p) => tabs.focusSession(p, sessionId));
  // TerminalPane focuses its terminal when its `focused` prop changes; when
  // the pane was already focused (or is mounted by this change), focus it
  // here once the DOM has updated, so keys do not stay on the sidebar.
  void tick().then(() => terminalControls(sessionId)?.focus());
}

export function focusDirection(direction: FocusDirection): void {
  const tab = currentTab();
  if (!tab) return;
  const target = focusNeighbor(layoutRects(tab.root, tab.zoomed).panes, tab.focused, direction);
  if (target) focusPane(target);
}

/**
 * Sends CreateSession with a client-chosen id and records where the session
 * goes when its SessionCreated arrives. `shell` overrides the program of a
 * shell session (Restart passes the old session's program); when omitted, a
 * shell uses the defaultShell setting.
 */
export function newSession(
  projectId: string,
  kind: SessionKind,
  placement: Placement = { kind: "tab" },
  shell?: string | null,
  resume: string | null = null,
): string {
  const id = newUlid();
  commit(requestCreate(app.workspace, id, { projectId, placement }));
  const machine = machineOfProject(projectId);
  const program = shellProgram(kind, shell, app.settings?.defaultShell, machine === LOCAL_MACHINE);
  void sendControl({ type: "CreateSession", session_id: id, project_id: projectId, kind, cols: 120, rows: 30, shell: program, resume }, machine).catch(() => {
    // Refused (for example, not connected): the reconnecting banner already
    // explains why. Stop waiting for a SessionCreated that is never coming.
    commit(createFailed(app.workspace, id));
  });
  return id;
}

/** `shell` is the program for a shell session; omitted, the defaultShell setting applies. */
export function splitSession(sessionId: string, direction: SplitDirection, kind: SessionKind, shell?: string): void {
  const projectId = projectIdOf(sessionId);
  if (!projectId) return;
  const tab = tabs.tabOf(projectLayout(projectId), sessionId);
  if (!tab || isFull(tab.root)) return;
  newSession(projectId, kind, { kind: "split", tabId: tab.id, besideId: sessionId, direction }, shell);
}

export function splitFocused(direction: SplitDirection, kind: SessionKind): void {
  const id = focusedSessionId();
  if (id) splitSession(id, direction, kind);
}

function sendClose(sessionId: string): void {
  void sendControl({ type: "CloseSession", session_id: sessionId }, machineOfSession(sessionId)).catch(() => {});
}

/** Spec 9.3 rule 6: closing a pane ends its session, after confirmation for a busy session. */
export async function closePane(sessionId: string): Promise<void> {
  const session = app.model.sessions[sessionId];
  if (!session) return;
  if (isBusy(session)) {
    const confirmed = await askConfirm({
      title: "Close pane?",
      message: `${displayLabel(session)} is ${busyStateText(session.state)}. Closing the pane ends the session.`,
      details: [],
      confirmLabel: "Close pane",
    });
    if (!confirmed) return;
  }
  sendClose(sessionId);
}

/** Spec 9.4, Close tab: lists the sessions it ends and how many are busy before confirming. */
export async function closeTab(tabId: string): Promise<void> {
  const tab = currentProjectLayout()?.tabs.find((t) => t.id === tabId);
  if (!tab) return;
  const sessions = paneIds(tab.root)
    .map((id) => app.model.sessions[id])
    .filter((s): s is SessionInfo => s !== undefined);
  const summary = closeTabSummary(sessions);
  if (closeTabNeedsConfirmation(summary)) {
    const confirmed = await askConfirm({ title: "Close tab?", message: closeTabMessage(summary), details: summary.labels, confirmLabel: "Close tab" });
    if (!confirmed) return;
  }
  // Each SessionRemoved removes one pane; the last one removes the tab.
  for (const session of sessions) sendClose(session.id);
}

export function renameTab(tabId: string, name: string): void {
  updateCurrentProject((p) => {
    const tab = p.tabs.find((t) => t.id === tabId);
    if (!tab) return p;
    // Committing the label the tab already shows keeps it following the
    // focused session instead of pinning that text as a name.
    if (tab.name === null && name.trim() === tabs.tabLabel(tab, (id) => { const s = app.model.sessions[id]; return s && displayLabel(s); })) return p;
    return tabs.renameTab(p, tabId, name);
  });
}

/** RenameSession is handled by jojod, which broadcasts SessionUpdated with the new label. */
export function renameSession(sessionId: string, label: string): void {
  const session = app.model.sessions[sessionId];
  const trimmed = label.trim();
  if (!session || trimmed === "" || trimmed === session.label) return;
  void sendControl({ type: "RenameSession", session_id: sessionId, label: trimmed }, machineOfSession(sessionId)).catch(() => {});
}

/** Kill ends the process; jojod keeps the row as exited so the final output stays readable. */
export function killSession(sessionId: string): void {
  void sendControl({ type: "Kill", session_id: sessionId }, machineOfSession(sessionId)).catch(() => {});
}

/**
 * Spec 5.4 Restart: a new session with the same project, kind and program
 * takes the old pane's place; the old session is closed when the new one's
 * SessionCreated arrives (workspace.sessionCreated reports it).
 */
export function restartSession(sessionId: string): void {
  const session = app.model.sessions[sessionId];
  if (!session || !canRestart(session)) return;
  newSession(session.project_id, session.kind, { kind: "replace", oldId: sessionId }, restartShell(session));
}

/** Spec 5.4: continues an exited claude session's conversation in its pane. */
export function resumeSession(sessionId: string): void {
  const session = app.model.sessions[sessionId];
  if (!session || !canResume(session)) return;
  newSession(session.project_id, "claude", { kind: "replace", oldId: sessionId }, null, session.claude_session_id);
}

export function zoomSession(sessionId: string): void {
  updateSessionProject(sessionId, (p) => tabs.toggleZoom(p, sessionId));
}

export function swapSessions(a: string, b: string): void {
  updateSessionProject(a, (p) => tabs.swapInTab(p, a, b));
}

export function moveSessionToNewTab(sessionId: string): void {
  updateSessionProject(sessionId, (p) => tabs.moveToNewTab(p, sessionId, newUlid()));
}

export function moveSessionToTab(sessionId: string, tabId: string): void {
  updateSessionProject(sessionId, (p) => tabs.moveToTab(p, sessionId, tabId));
}

/** Sidebar menu, Open in split: moves the session into the current tab beside its focused pane. */
export function openSessionInSplit(sessionId: string, direction: SplitDirection): void {
  const tab = currentTab();
  if (!tab || projectIdOf(sessionId) !== app.selectedProjectId) return;
  updateSessionProject(sessionId, (p) => tabs.moveBeside(p, sessionId, tab.id, tab.focused, direction));
}

export function applyTabPreset(tabId: string, preset: LayoutPreset): void {
  updateCurrentProject((p) => tabs.presetTab(p, tabId, preset));
}

export function equalizeTabSizes(tabId: string): void {
  updateCurrentProject((p) => tabs.equalizeTab(p, tabId));
}

export function setTabRoot(tabId: string, root: LayoutNode): void {
  updateCurrentProject((p) => tabs.setTabRoot(p, tabId, root));
}

/** A machine's projects in sidebar order (layout.json's project_order). */
export function projectsInOrder(machineId: string): ProjectInfo[] {
  return orderedProjects(app.models[machineId] ?? emptyModel(), machineId, app.workspace.layout.project_order ?? []);
}

/** Moves a project of `machineId` to a drop position in its sidebar list; saved in layout.json. */
export function reorderProject(machineId: string, projectId: string, toIndex: number): void {
  const shown = projectsInOrder(machineId).map((p) => p.id);
  const project_order = reorderedProjectOrder(app.workspace.layout.project_order ?? [], machineId, shown, projectId, toIndex);
  commit({ ...app.workspace, layout: { ...app.workspace.layout, project_order } });
}

/** How long removeProject waits for the project's running sessions to end. */
const REMOVE_PROJECT_WAIT_MS = 10_000;

/**
 * Removes a project from the sidebar and from its machine's jojod, after a
 * confirmation that names the running sessions it ends. The folder on disk
 * is not touched. jojod refuses RemoveProject while a session of the
 * project runs, so those sessions are closed first and the request is sent
 * once none is left (or after REMOVE_PROJECT_WAIT_MS, when jojod's refusal
 * shows in the banner). Exited sessions are removed with the project.
 */
export async function removeProject(projectId: string): Promise<void> {
  const project = app.model.projects[projectId];
  if (!project) return;
  const machine = machineOfProject(projectId);
  const runningOf = () => sessionsForProject(app.model, projectId).filter((s) => s.state !== "exited");
  const running = runningOf();
  const ends = running.length === 0 ? "" : ` It ends ${running.length} running session${running.length === 1 ? "" : "s"}.`;
  const confirmed = await askConfirm({
    title: `Remove ${project.name}?`,
    message: `Removes the project from Jojo.${ends} The folder ${project.path} and its files are not deleted; add it again with + to bring it back.`,
    details: running.map((s) => displayLabel(s)),
    confirmLabel: "Remove project",
  });
  if (!confirmed) return;
  for (const session of running) sendClose(session.id);
  const deadline = Date.now() + REMOVE_PROJECT_WAIT_MS;
  while (runningOf().length > 0 && Date.now() < deadline) await new Promise((resolve) => setTimeout(resolve, 100));
  if (app.selectedProjectId === projectId) app.selectedProjectId = null;
  void sendControl({ type: "RemoveProject", id: projectId }, machine).catch(() => {});
}

export async function addProject(): Promise<void> {
  const path = await open({ directory: true, multiple: false });
  if (typeof path !== "string") return;
  const name = path.split(/[\\/]/).filter(Boolean).pop() ?? path;
  void sendControl({ type: "AddProject", path, name }).catch(() => {});
}

/**
 * Adds a project on a remote machine: `path` is an absolute path on that
 * machine. The daemon checks that it is a directory and replies with an
 * Error otherwise, which the banner shows.
 */
export function addRemoteProject(machineId: string, path: string): Promise<void> {
  const trimmed = path.trim().replace(/\/+$/, "") || "/";
  const last = trimmed.split("/").filter(Boolean).pop() ?? trimmed;
  const name = last === "~" ? "home" : last;
  return sendControl({ type: "AddProject", path: trimmed, name }, machineId);
}

/**
 * Spec 8.4, remote mismatch: upgrades jojod on a machine whose daemon speaks
 * another protocol. The confirmation names the sessions last seen there,
 * because the old daemon is shut down and they end.
 */
export async function upgradeMachine(machineId: string): Promise<void> {
  const machine = app.machines.find((m) => m.id === machineId);
  if (!machine?.ssh_host) return;
  const sessions = Object.values(app.models[machineId]?.sessions ?? {}).filter((s) => s.state !== "exited");
  const confirmed = await askConfirm({
    title: `Upgrade jojod on ${machine.label}?`,
    message: `The running jojod speaks another protocol version. Upgrading shuts it down, which ends every session running on ${machine.label}${sessions.length ? ` (${sessions.length} last seen)` : ""}, then installs and starts this version.`,
    details: sessions.map((s) => s.label),
    confirmLabel: "Upgrade",
  });
  if (!confirmed) return;
  const progress = new Channel<unknown>();
  try {
    await invoke("add_machine", { host: machine.ssh_host, label: machine.label, allowRestart: true, progress });
  } catch (err) {
    app.models[LOCAL_MACHINE] = { ...(app.models[LOCAL_MACHINE] ?? emptyModel()), lastError: { request: "Upgrade", message: errorText(err), session_id: null } };
    mergeModels();
  }
}

/** Asks a machine for the subfolders of `path` (answered through app.dirEntries). */
export function listDir(machineId: string, path: string): void {
  void sendControl({ type: "ListDir", path }, machineId).catch(() => {});
}

/** Lists a new machine in the sidebar (add_machine has saved it and started its connection). */
export function machineAdded(machine: Machine): void {
  if (!app.machines.some((m) => m.id === machine.id)) app.machines = [...app.machines, machine];
}

/**
 * Spec 8.4: removes a remote machine after confirmation, which names the
 * sessions that end when `uninstall` is set (otherwise they keep running on
 * the server, unreachable from this PC).
 */
export async function removeMachine(machineId: string, uninstall: boolean): Promise<void> {
  const machine = app.machines.find((m) => m.id === machineId);
  if (!machine || machine.kind === "local") return;
  const sessions = Object.values(app.models[machineId]?.sessions ?? {}).filter((s) => s.state !== "exited");
  const confirmed = await askConfirm({
    title: uninstall ? `Remove ${machine.label} and uninstall Jojo from it?` : `Remove ${machine.label}?`,
    message: uninstall
      ? `This stops jojod on the server, which ends ${sessions.length} running session${sessions.length === 1 ? "" : "s"}, removes Jojo's Claude Code hooks there, and deletes ~/.local/bin/jojod and ~/.jojo.`
      : `Jojo disconnects from the server and forgets it. Its ${sessions.length} running session${sessions.length === 1 ? " keeps" : "s keep"} running on the server until the server restarts.`,
    details: sessions.map((s) => s.label),
    confirmLabel: uninstall ? "Remove and uninstall" : "Remove",
  });
  if (!confirmed) return;
  try {
    await invoke("remove_machine", { id: machineId, uninstall });
  } catch (err) {
    app.models[LOCAL_MACHINE] = { ...(app.models[LOCAL_MACHINE] ?? emptyModel()), lastError: { request: "RemoveMachine", message: errorText(err), session_id: null } };
    mergeModels();
    return;
  }
  app.machines = app.machines.filter((m) => m.id !== machineId);
  if (app.selectedProjectId && machineOfProject(app.selectedProjectId) === machineId) app.selectedProjectId = null;
  delete app.models[machineId];
  delete app.statuses[machineId];
  delete app.loaded[machineId];
  mergeModels();
  if (!app.selectedProjectId) app.selectedProjectId = sortedProjects(app.model)[0]?.id ?? null;
  const prefix = `${machineId}:`;
  const projects = Object.fromEntries(Object.entries(app.workspace.layout.projects).filter(([key]) => !key.startsWith(prefix)));
  commit({ ...app.workspace, layout: { ...app.workspace.layout, projects } });
}

export function dismissError(): void {
  for (const [machine, model] of Object.entries(app.models)) {
    if (model.lastError) app.models[machine] = { ...model, lastError: null };
  }
  mergeModels();
}

/** Saves the settings screen's values and applies the terminal font to open panes (spec 9.10). */
export async function saveSettings(settings: Settings): Promise<void> {
  await invoke("save_settings", { settings });
  app.settings = settings;
  // The new font's faces load before the panes measure their cells with it.
  await loadTerminalFont(settings.terminalFontFamily);
  for (const terminal of allTerminals()) {
    terminal.applyFont(terminalFontStack(settings.terminalFontFamily), settings.terminalFontSize, settings.terminalLineHeight);
  }
}

/** Running sessions on a machine (the ones a daemon restart ends). */
export function runningSessions(machine: string): SessionInfo[] {
  return Object.values(app.models[machine]?.sessions ?? {}).filter((s) => s.state !== "exited");
}

/** Spec 9.10 and 8.4: restarts This PC's jojod after a confirmation that names the sessions it ends. */
export async function confirmRestartLocalDaemon(): Promise<void> {
  // While This PC's jojod speaks another protocol the app has never listed
  // its sessions; their number comes from its state.json (labels unknown),
  // and the restart is always confirmed.
  const mismatch = app.status.state === "version_mismatch";
  const sessions = mismatch ? [] : runningSessions(LOCAL_MACHINE);
  const count = mismatch ? await invoke<number | null>("local_running_sessions").catch(() => null) : sessions.length;
  const confirmed =
    (!mismatch && count === 0) ||
    (await askConfirm({
      title: `Restart jojod on ${LOCAL_LABEL}?`,
      message:
        count === null
          ? "Restarting ends every session the running jojod has."
          : `Restarting ends ${count} running session${count === 1 ? "" : "s"}${sessions.length ? ":" : "."}`,
      details: sessions.map((s) => s.label),
      confirmLabel: "Restart jojod",
    }));
  if (confirmed) restartDaemon(LOCAL_MACHINE);
}

/** Spec 10, Debug ▸ Rehydrate pane: a new snapshot for the focused pane. */
export function rehydrateFocusedPane(): void {
  terminalControls(focusedSessionId())?.rehydrate();
}

/** This PC: restarts jojod. A remote machine: reconnects now. */
export function restartDaemon(machine: string = LOCAL_MACHINE): void {
  void invoke("restart_daemon", { machine }).catch(() => {});
}
