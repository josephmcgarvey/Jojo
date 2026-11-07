// The app's view of the daemon, built only from control messages.
// Events are applied as idempotent upserts; events about unknown ids are
// ignored (Plan 2a carry-over 2.1, item 1).
import type { ControlMsg } from "../protocol/ControlMsg";
import type { ProjectInfo } from "../protocol/ProjectInfo";
import type { SessionInfo } from "../protocol/SessionInfo";

export interface Model {
  projects: Record<string, ProjectInfo>;
  sessions: Record<string, SessionInfo>;
  lastError: { request: string; message: string; session_id: string | null } | null;
}

export function emptyModel(): Model {
  return { projects: {}, sessions: {}, lastError: null };
}

function without<T>(record: Record<string, T>, key: string): Record<string, T> {
  const { [key]: _removed, ...rest } = record;
  return rest;
}

export function applyControl(model: Model, msg: ControlMsg): Model {
  switch (msg.type) {
    case "Projects":
      return { ...model, projects: Object.fromEntries(msg.projects.map((p) => [p.id, p])) };
    case "ProjectAdded":
      return { ...model, projects: { ...model.projects, [msg.project.id]: msg.project } };
    case "ProjectRemoved":
      return { ...model, projects: without(model.projects, msg.id) };
    case "Sessions":
      return { ...model, sessions: Object.fromEntries(msg.sessions.map((s) => [s.id, s])) };
    case "SessionCreated":
    case "SessionUpdated":
      return { ...model, sessions: { ...model.sessions, [msg.session.id]: msg.session } };
    case "SessionRemoved":
      return { ...model, sessions: without(model.sessions, msg.session_id) };
    case "SessionExited": {
      const current = model.sessions[msg.session_id];
      if (!current) return model;
      return {
        ...model,
        sessions: { ...model.sessions, [current.id]: { ...current, state: "exited", exit_code: msg.exit_code } },
      };
    }
    case "Error":
      return { ...model, lastError: { request: msg.request, message: msg.message, session_id: msg.session_id } };
    default:
      return model;
  }
}

const CONNECTION_ERROR_REQUESTS = new Set(["Hello", "Events"]);

/**
 * Errors about the connection itself rather than about a request the user
 * made: jojod sends "Hello" when the protocol versions differ and "Events"
 * when this client fell behind on events, and closes the connection after
 * either. A successful reconnect resolves both, so they are cleared when
 * the status becomes "connected". Any other error is left for the user to
 * dismiss.
 */
export function clearConnectionErrors(model: Model): Model {
  if (!model.lastError || !CONNECTION_ERROR_REQUESTS.has(model.lastError.request)) return model;
  return { ...model, lastError: null };
}
