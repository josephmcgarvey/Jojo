// The app's reactive state and actions. The rules live in pure modules
// (../layout, ./workspace, ./sessionRules); this module holds the current
// values, talks to the backend, and keeps layout.json in sync.
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { tick } from "svelte";
import type { ConnectionStatus } from "../backend/ConnectionStatus";
import type { LayoutFile } from "../backend/LayoutFile";
import type { ProjectLayout } from "../backend/ProjectLayout";
import type { Settings } from "../backend/Settings";
import type { ShellOption } from "../backend/ShellOption";
import type { TabLayout } from "../backend/TabLayout";
import { errorText } from "../errors";
import { focusNeighbor, layoutRects, type FocusDirection } from "../layout/geometry";
import { reconcileAll } from "../layout/reconcile";
import * as tabs from "../layout/tabs";
import { isFull, paneIds, type LayoutNode, type LayoutPreset, type SplitDirection } from "../layout/tree";
import type { ControlMsg } from "../protocol/ControlMsg";
import type { SessionInfo } from "../protocol/SessionInfo";
import type { SessionKind } from "../protocol/SessionKind";
import { router } from "../terminal/router";
import { sendControl, subscribe } from "../wire/bridge";
import { newUlid } from "../wire/ulid";
import { askConfirm } from "./dialog.svelte";
import { terminalControls } from "../terminal/registry";
import { applyControl, clearConnectionErrors, emptyModel, type Model } from "./reducer";
import {
  busyStateText,
  canRestart,
  canResume,
  closeTabMessage,
  closeTabNeedsConfirmation,
  closeTabSummary,
  closesOnExit,
  isBusy,
  restartShell,
} from "./sessionRules";
import { sortedProjects } from "./views";
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
  model: emptyModel() as Model,
  status: { state: "starting" } as ConnectionStatus,
  /** True once any connection to jojod has succeeded in this app run. */
  everConnected: false,
  settings: null as Settings | null,
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
});

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
    })
    .catch(() => {
      // Settings stay at their built-in defaults; nothing in the UI depends
      // on this having succeeded.
    });
  // layout.json is read before subscribing, so no SessionCreated can be
  // placed into an empty layout that the file then overwrites.
  void loadLayout().then(connect);
  const tick = setInterval(() => (app.now = Date.now()), 30_000);
  return () => clearInterval(tick);
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
    onStatus(status) {
      app.status = status;
      if (status.state === "connected") {
        app.everConnected = true;
        app.model = clearConnectionErrors(app.model);
        void sendControl({ type: "ListProjects" }).catch(() => {});
        void sendControl({ type: "ListSessions" }).catch(() => {});
      }
    },
    onFrame(frame) {
      if (frame.kind === "control") handleControl(frame.msg);
      else router.deliver(frame);
    },
  }).catch((err: unknown) => {
    // The bridge itself failed to register (not merely "disconnected", which
    // arrives as a status update instead): there is no connection to retry
    // from in-app, so show it the same way a failed daemon launch would be.
    app.status = { state: "failed", message: errorText(err) };
  });
}

function reconcile(): void {
  // reconcileAll drops the layout of every project missing from the model,
  // so it must not run before the first Projects reply: ListProjects and
  // ListSessions are separate IPC calls, and Sessions can arrive first.
  if (!app.layoutLoaded || !app.projectsLoaded || !app.sessionsLoaded) return;
  commit({ ...app.workspace, layout: reconcileAll(app.workspace.layout, app.model, newUlid) });
}

function handleControl(msg: ControlMsg): void {
  if (msg.type === "Error" && msg.request === "Output") {
    router.fellBehind(msg.session_id);
    return;
  }
  if (msg.type === "Error" && msg.session_id) {
    if (msg.request === "Attach") router.attachFailed(msg.session_id);
    if (msg.request === "CreateSession") commit(createFailed(app.workspace, msg.session_id));
  }

  app.model = applyControl(app.model, msg);

  switch (msg.type) {
    case "Sessions":
      app.sessionsLoaded = true;
      reconcile();
      break;
    case "Projects":
      app.projectsLoaded = true;
      reconcile();
      break;
    case "ProjectRemoved":
      reconcile();
      break;
    case "SessionCreated": {
      const result = sessionCreated(app.workspace, msg.session, newUlid);
      commit(result.workspace);
      if (result.requested) app.selectedProjectId = msg.session.project_id;
      if (result.closeSessionId) sendClose(result.closeSessionId);
      break;
    }
    case "SessionExited":
      // Plan 3 amendment A: an exit with code 0 (for example `exit` in a
      // shell) closes the pane; any other code keeps the final output
      // readable (spec 5.3). SessionExited is an event, so this applies only
      // to exits observed while connected; sessions already exited in a
      // Sessions reply are left for the user to close.
      if (closesOnExit(msg.exit_code)) sendClose(msg.session_id);
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
  if (msg.type === "Sessions") router.reattachAll();
}

export function projectLayout(projectId: string): ProjectLayout {
  return projectOf(app.workspace.layout, projectId);
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
  const layout = withProject(app.workspace.layout, projectId, update);
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
): void {
  const id = newUlid();
  commit(requestCreate(app.workspace, id, { projectId, placement }));
  const program = shell !== undefined ? shell : kind === "shell" ? (app.settings?.defaultShell ?? null) : null;
  void sendControl({ type: "CreateSession", session_id: id, project_id: projectId, kind, cols: 120, rows: 30, shell: program, resume }).catch(() => {
    // Refused (for example, not connected): the reconnecting banner already
    // explains why. Stop waiting for a SessionCreated that is never coming.
    commit(createFailed(app.workspace, id));
  });
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
  void sendControl({ type: "CloseSession", session_id: sessionId }).catch(() => {});
}

/** Spec 9.3 rule 6: closing a pane ends its session, after confirmation for a busy session. */
export async function closePane(sessionId: string): Promise<void> {
  const session = app.model.sessions[sessionId];
  if (!session) return;
  if (isBusy(session)) {
    const confirmed = await askConfirm({
      title: "Close pane?",
      message: `${session.label} is ${busyStateText(session.state)}. Closing the pane ends the session.`,
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
    if (tab.name === null && name.trim() === tabs.tabLabel(tab, (id) => app.model.sessions[id]?.label)) return p;
    return tabs.renameTab(p, tabId, name);
  });
}

/** RenameSession is handled by jojod, which broadcasts SessionUpdated with the new label. */
export function renameSession(sessionId: string, label: string): void {
  const session = app.model.sessions[sessionId];
  const trimmed = label.trim();
  if (!session || trimmed === "" || trimmed === session.label) return;
  void sendControl({ type: "RenameSession", session_id: sessionId, label: trimmed }).catch(() => {});
}

/** Kill ends the process; jojod keeps the row as exited so the final output stays readable. */
export function killSession(sessionId: string): void {
  void sendControl({ type: "Kill", session_id: sessionId }).catch(() => {});
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

export async function addProject(): Promise<void> {
  const path = await open({ directory: true, multiple: false });
  if (typeof path !== "string") return;
  const name = path.split(/[\\/]/).filter(Boolean).pop() ?? path;
  void sendControl({ type: "AddProject", path, name }).catch(() => {});
}

export function dismissError(): void {
  app.model = { ...app.model, lastError: null };
}

export function restartDaemon(): void {
  void invoke("restart_daemon").catch(() => {});
}
