import type { ProjectInfo } from "../protocol/ProjectInfo";
import type { SessionInfo } from "../protocol/SessionInfo";
import type { Model } from "./reducer";

export function sessionsForProject(model: Model, projectId: string): SessionInfo[] {
  return Object.values(model.sessions)
    .filter((s) => s.project_id === projectId)
    .sort((a, b) => a.created_at - b.created_at);
}

/**
 * Every session in sidebar order: machines in machines.toml order, each
 * machine's projects by name, each project's sessions by creation time.
 */
export function orderedSessions(
  machines: { id: string }[],
  models: Record<string, Model>,
): { session: SessionInfo; machineId: string }[] {
  const out: { session: SessionInfo; machineId: string }[] = [];
  for (const machine of machines) {
    const model = models[machine.id];
    if (!model) continue;
    for (const project of sortedProjects(model)) {
      for (const session of sessionsForProject(model, project.id)) out.push({ session, machineId: machine.id });
    }
  }
  return out;
}

export function sortedProjects(model: Model): ProjectInfo[] {
  return Object.values(model.projects).sort((a, b) => a.name.localeCompare(b.name));
}
