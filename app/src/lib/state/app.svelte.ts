// The app's reactive state and actions.
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { ConnectionStatus } from "../backend/ConnectionStatus";
import type { Settings } from "../backend/Settings";
import type { ControlMsg } from "../protocol/ControlMsg";
import type { SessionInfo } from "../protocol/SessionInfo";
import type { SessionKind } from "../protocol/SessionKind";
import { router } from "../terminal/router";
import { sendControl, subscribe } from "../wire/bridge";
import { newUlid } from "../wire/ulid";
import { applyControl, clearConnectionErrors, emptyModel, type Model } from "./reducer";
import { nextActiveAfterRemoval, sessionsForProject, sortedProjects } from "./views";

export const app = $state({
  model: emptyModel() as Model,
  status: { state: "starting" } as ConnectionStatus,
  settings: null as Settings | null,
  selectedProjectId: null as string | null,
  activeSessionId: null as string | null,
  pendingSessionId: null as string | null,
  now: Date.now(),
});

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

export function startApp(): () => void {
  void invoke<Settings>("get_settings")
    .then((s) => (app.settings = s))
    .catch(() => {
      // Settings stay at their built-in defaults; nothing in the UI depends
      // on this having succeeded.
    });
  void subscribe({
    onStatus(status) {
      app.status = status;
      if (status.state === "connected") {
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
    app.status = { state: "failed", message: errorMessage(err) };
  });
  const tick = setInterval(() => (app.now = Date.now()), 30_000);
  return () => clearInterval(tick);
}

function handleControl(msg: ControlMsg): void {
  if (msg.type === "Error" && msg.request === "Output") {
    router.fellBehind(msg.session_id);
    return;
  }

  // Computed against the pre-update model, which still has the removed
  // session and its siblings, so the project's creation order is intact.
  const activeRemoved = msg.type === "SessionRemoved" && msg.session_id === app.activeSessionId;
  const reactivateAfterRemoval = activeRemoved ? nextActiveAfterRemoval(app.model, msg.session_id) : null;

  app.model = applyControl(app.model, msg);

  if (activeRemoved) {
    app.activeSessionId = reactivateAfterRemoval;
  }

  if (msg.type === "SessionCreated" && msg.session.id === app.pendingSessionId) {
    app.pendingSessionId = null;
    activateSession(msg.session);
  }

  if (!app.selectedProjectId || !app.model.projects[app.selectedProjectId]) {
    const first = sortedProjects(app.model)[0];
    if (first) selectProject(first.id);
  }

  if (app.activeSessionId && !app.model.sessions[app.activeSessionId]) {
    app.activeSessionId = app.selectedProjectId ? (sessionsForProject(app.model, app.selectedProjectId)[0]?.id ?? null) : null;
  }

  // Projects can arrive before Sessions on startup, which leaves
  // activeSessionId null even though the selected project has sessions once
  // Sessions is applied above. Auto-activate only on the two events that
  // actually introduce sessions to look at (the full list, and a session
  // appearing in the selected project with nothing already active) so that
  // an explicit user action, like closing the last tab, is not undone here;
  // closing a tab is handled by the removal fallback above instead.
  if (!app.activeSessionId && app.selectedProjectId) {
    const projectId = app.selectedProjectId;
    const introducesSessions = msg.type === "Sessions" || (msg.type === "SessionCreated" && msg.session.project_id === projectId);
    if (introducesSessions) {
      app.activeSessionId = sessionsForProject(app.model, projectId)[0]?.id ?? null;
    }
  }

  // Sessions carries every session's current attachable value; reattach only
  // once it has been applied, so a reconnect does not race an Attach against
  // stale attachable state from before the daemon restarted.
  if (msg.type === "Sessions") {
    router.reattachAll();
  }
}

export function selectProject(id: string): void {
  app.selectedProjectId = id;
  app.activeSessionId = sessionsForProject(app.model, id)[0]?.id ?? null;
}

export function activateSession(session: SessionInfo): void {
  app.selectedProjectId = session.project_id;
  app.activeSessionId = session.id;
}

export function newSession(projectId: string, kind: SessionKind): void {
  const id = newUlid();
  app.pendingSessionId = id;
  void sendControl({
    type: "CreateSession",
    session_id: id,
    project_id: projectId,
    kind,
    cols: 120,
    rows: 30,
    shell: kind === "shell" ? (app.settings?.defaultShell ?? null) : null,
  }).catch(() => {
    // Refused (for example, not connected): the reconnecting banner already
    // explains why, so there is nothing more to surface here. Just stop
    // waiting on a SessionCreated that is never coming.
    if (app.pendingSessionId === id) app.pendingSessionId = null;
  });
}

export function closeSession(id: string): void {
  void sendControl({ type: "CloseSession", session_id: id }).catch(() => {});
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
