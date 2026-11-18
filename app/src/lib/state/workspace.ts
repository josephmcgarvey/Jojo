// The layout of every project plus the session creates still waiting for
// their SessionCreated reply. Pure; app.svelte.ts holds the current value.
import type { LayoutFile } from "../backend/LayoutFile";
import type { ProjectLayout } from "../backend/ProjectLayout";
import { layoutKey, LOCAL_MACHINE_ID } from "../layout/reconcile";
import { addTab, emptyProject, newTab, removeSession, replaceSession, splitBeside, tabOf } from "../layout/tabs";
import type { SplitDirection } from "../layout/tree";
import type { SessionInfo } from "../protocol/SessionInfo";

/** Format version of layout.json; equals `LAYOUT_VERSION` in app/src-tauri/src/layout.rs. */
export const LAYOUT_VERSION = 1;

/** Where a session the user asked for goes when its SessionCreated arrives. */
export type Placement =
  | { kind: "tab" }
  | { kind: "split"; tabId: string; besideId: string; direction: SplitDirection }
  | { kind: "replace"; oldId: string };

export interface PendingCreate {
  projectId: string;
  placement: Placement;
}

export interface Workspace {
  layout: LayoutFile;
  /** Keyed by the client-chosen session id sent in CreateSession. */
  pending: Record<string, PendingCreate>;
}

export function emptyWorkspace(): Workspace {
  return { layout: { version: LAYOUT_VERSION, projects: {}, project_order: [], collapsed: [], flagged: [] }, pending: {} };
}

/** The stored layout of a project on `machineId` (This PC by default). */
export function projectOf(layout: LayoutFile, projectId: string, machineId: string = LOCAL_MACHINE_ID): ProjectLayout {
  return layout.projects[layoutKey(machineId, projectId)] ?? emptyProject();
}

/**
 * Applies `update` to the layout of a project on `machineId` (This PC by
 * default). Returns the same LayoutFile when `update` returns its input
 * unchanged, and null when `update` returns null (the operation was refused).
 */
export function withProject(
  layout: LayoutFile,
  projectId: string,
  update: (project: ProjectLayout) => ProjectLayout | null,
  machineId: string = LOCAL_MACHINE_ID,
): LayoutFile | null {
  const key = layoutKey(machineId, projectId);
  const current = layout.projects[key] ?? emptyProject();
  const next = update(current);
  if (next === null) return null;
  if (next === current) return layout;
  return { ...layout, projects: { ...layout.projects, [key]: next } };
}

export function requestCreate(ws: Workspace, sessionId: string, request: PendingCreate): Workspace {
  return { ...ws, pending: { ...ws.pending, [sessionId]: request } };
}

/** Forgets a pending create (the request was refused, or it has been placed). */
export function createFailed(ws: Workspace, sessionId: string): Workspace {
  if (!(sessionId in ws.pending)) return ws;
  const { [sessionId]: _placed, ...pending } = ws.pending;
  return { ...ws, pending };
}

export interface CreatedResult {
  workspace: Workspace;
  /** True when this app asked for the session; the caller then selects its project. */
  requested: boolean;
  /** For a restart: the session the new one replaced, which the caller closes. */
  closeSessionId: string | null;
}

/**
 * Places a session reported by SessionCreated. A requested session goes
 * where it was requested and its tab becomes active. A split whose tab is
 * now full or gone, or a replacement whose old session is gone, falls back
 * to a new tab. A session nobody requested gets a new tab that is not
 * activated. A session that is already placed is left where it is.
 */
export function sessionCreated(
  ws: Workspace,
  session: SessionInfo,
  newTabId: () => string,
  machineId: string = LOCAL_MACHINE_ID,
): CreatedResult {
  const request = ws.pending[session.id];
  const requested = request !== undefined;
  const base = createFailed(ws, session.id);
  if (tabOf(projectOf(base.layout, session.project_id, machineId), session.id)) {
    return { workspace: base, requested, closeSessionId: null };
  }
  const placement: Placement = request?.placement ?? { kind: "tab" };
  const asNewTab = (project: ProjectLayout): ProjectLayout => addTab(project, newTab(newTabId(), session.id), requested);
  const closeSessionId = placement.kind === "replace" ? placement.oldId : null;
  const layout = withProject(base.layout, session.project_id, (project) => {
    switch (placement.kind) {
      case "split":
        return splitBeside(project, placement.tabId, placement.besideId, session.id, placement.direction) ?? asNewTab(project);
      case "replace":
        return replaceSession(project, placement.oldId, session.id) ?? asNewTab(project);
      case "tab":
        return asNewTab(project);
    }
  }, machineId);
  return { workspace: { ...base, layout: layout ?? base.layout }, requested, closeSessionId };
}

/** Removes a closed session's pane from whichever local project holds it. */
export function sessionRemoved(ws: Workspace, sessionId: string): Workspace {
  let changed = false;
  const projects: LayoutFile["projects"] = { ...ws.layout.projects };
  for (const [key, project] of Object.entries(ws.layout.projects)) {
    if (!project || !tabOf(project, sessionId)) continue;
    projects[key] = removeSession(project, sessionId);
    changed = true;
  }
  return changed ? { ...ws, layout: { ...ws.layout, projects } } : ws;
}
