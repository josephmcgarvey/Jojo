import type { ProjectInfo } from "../protocol/ProjectInfo";
import type { SessionInfo } from "../protocol/SessionInfo";
import type { Model } from "./reducer";

export function sessionsForProject(model: Model, projectId: string): SessionInfo[] {
  return Object.values(model.sessions)
    .filter((s) => s.project_id === projectId)
    .sort((a, b) => a.created_at - b.created_at);
}

export function sortedProjects(model: Model): ProjectInfo[] {
  return Object.values(model.projects).sort((a, b) => a.name.localeCompare(b.name));
}

/**
 * Which session should become active when `removedId` is removed, given the
 * model from *before* that removal is applied (so `removedId` is still
 * present and still has its siblings). Picks the session after it in the
 * project's creation order, falls back to the one before it if it was last,
 * and returns `null` if it was the project's only session.
 */
export function nextActiveAfterRemoval(model: Model, removedId: string): string | null {
  const removed = model.sessions[removedId];
  if (!removed) return null;
  const siblings = sessionsForProject(model, removed.project_id);
  const index = siblings.findIndex((s) => s.id === removedId);
  if (index < 0) return null;
  return (siblings[index + 1] ?? siblings[index - 1])?.id ?? null;
}
