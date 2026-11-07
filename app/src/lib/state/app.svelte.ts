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
import { applyControl, emptyModel, type Model } from "./reducer";
import { sessionsForProject, sortedProjects } from "./views";

export const app = $state({
  model: emptyModel() as Model,
  status: { state: "starting" } as ConnectionStatus,
  settings: null as Settings | null,
  selectedProjectId: null as string | null,
  activeSessionId: null as string | null,
  pendingSessionId: null as string | null,
  now: Date.now(),
});

export function startApp(): () => void {
  void invoke<Settings>("get_settings").then((s) => (app.settings = s));
  void subscribe({
    onStatus(status) {
      app.status = status;
      if (status.state === "connected") {
        void sendControl({ type: "ListProjects" });
        void sendControl({ type: "ListSessions" });
        router.reattachAll();
      }
    },
    onFrame(frame) {
      if (frame.kind === "control") handleControl(frame.msg);
      else router.deliver(frame);
    },
  });
  const tick = setInterval(() => (app.now = Date.now()), 30_000);
  return () => clearInterval(tick);
}

function handleControl(msg: ControlMsg): void {
  if (msg.type === "Error" && msg.request === "Output") {
    router.fellBehind(msg.session_id);
    return;
  }
  app.model = applyControl(app.model, msg);
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
  });
}

export function closeSession(id: string): void {
  void sendControl({ type: "CloseSession", session_id: id });
}

export async function addProject(): Promise<void> {
  const path = await open({ directory: true, multiple: false });
  if (typeof path !== "string") return;
  const name = path.split(/[\\/]/).filter(Boolean).pop() ?? path;
  void sendControl({ type: "AddProject", path, name });
}

export function dismissError(): void {
  app.model = { ...app.model, lastError: null };
}

export function restartDaemon(): void {
  void invoke("restart_daemon");
}
