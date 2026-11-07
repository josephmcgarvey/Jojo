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
