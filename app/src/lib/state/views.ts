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

/**
 * A machine's projects in sidebar order: the ones listed in `order`
 * (layout.json's project_order, `<machine>:<project>` keys) in that order,
 * then the others sorted by name.
 */
export function orderedProjects(model: Model, machineId: string, order: readonly string[]): ProjectInfo[] {
  const rank = new Map(order.map((key, index) => [key, index]));
  const rankOf = (p: ProjectInfo) => rank.get(`${machineId}:${p.id}`) ?? Infinity;
  return sortedProjects(model).sort((a, b) => {
    const ra = rankOf(a);
    const rb = rankOf(b);
    return ra === rb ? 0 : ra < rb ? -1 : 1;
  });
}

/**
 * The project_order after dragging `projectId` of `machineId` so it sits
 * before the project now at `toIndex` in `shown` (the machine's projects as
 * the sidebar lists them; shown.length puts it last). The machine's keys are
 * replaced by its full new order, so every one of its projects is listed
 * from then on; other machines' keys are kept.
 */
export function reorderedProjectOrder(order: readonly string[], machineId: string, shown: readonly string[], projectId: string, toIndex: number): string[] {
  const from = shown.indexOf(projectId);
  if (from < 0) return [...order];
  const target = Math.max(0, Math.min(shown.length, Math.round(toIndex)));
  const insertAt = target > from ? target - 1 : target;
  const ids = shown.filter((id) => id !== projectId);
  ids.splice(insertAt, 0, projectId);
  const prefix = `${machineId}:`;
  return [...order.filter((key) => !key.startsWith(prefix)), ...ids.map((id) => prefix + id)];
}

export function sortedProjects(model: Model): ProjectInfo[] {
  return Object.values(model.projects).sort((a, b) => a.name.localeCompare(b.name));
}
