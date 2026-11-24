import type { ProjectInfo } from "../protocol/ProjectInfo";
import type { SessionInfo } from "../protocol/SessionInfo";
import type { Model } from "./reducer";

export function sessionsForProject(model: Model, projectId: string): SessionInfo[] {
  return Object.values(model.sessions)
    .filter((s) => s.project_id === projectId)
    .sort((a, b) => a.created_at - b.created_at);
}

/**
 * The dragged sidebar order, from layout.json: `machine_order` (location
 * ids), `project_order` (`<machine>:<project>` keys) and `session_order`
 * (`<machine>:<session>` keys). Absent lists mean the default order.
 */
export interface SidebarOrder {
  machine_order?: readonly string[];
  project_order?: readonly string[];
  session_order?: readonly string[];
}

/** `items` with the ones whose key is listed in `order` first, in that order; the rest keep their order after them. */
function byRank<T>(items: T[], order: readonly string[], keyOf: (item: T) => string): T[] {
  if (order.length === 0) return items;
  const rank = new Map(order.map((key, index) => [key, index]));
  return items
    .map((item, index) => ({ item, index, rank: rank.get(keyOf(item)) ?? Infinity }))
    .sort((a, b) => (a.rank === b.rank ? a.index - b.index : a.rank < b.rank ? -1 : 1))
    .map((x) => x.item);
}

/** The locations in sidebar order: the ones listed in `order` (layout.json's machine_order) in that order, then the others in machines.toml order. */
export function orderedMachines<M extends { id: string }>(machines: M[], order: readonly string[]): M[] {
  return byRank(machines, order, (m) => m.id);
}

/** A project's sessions in sidebar order: the ones listed in `order` (layout.json's session_order) in that order, then the others by creation time. */
export function orderedProjectSessions(model: Model, machineId: string, projectId: string, order: readonly string[]): SessionInfo[] {
  return byRank(sessionsForProject(model, projectId), order, (s) => `${machineId}:${s.id}`);
}

/**
 * Every session in sidebar order: locations, each location's projects and
 * each project's sessions in their dragged order (`order`), with the ones
 * never dragged in their default order (machines.toml order, projects by
 * name, sessions by creation time).
 */
export function orderedSessions(
  machines: { id: string }[],
  models: Record<string, Model>,
  order: SidebarOrder = {},
): { session: SessionInfo; machineId: string }[] {
  const out: { session: SessionInfo; machineId: string }[] = [];
  for (const machine of orderedMachines(machines, order.machine_order ?? [])) {
    const model = models[machine.id];
    if (!model) continue;
    for (const project of orderedProjects(model, machine.id, order.project_order ?? [])) {
      for (const session of orderedProjectSessions(model, machine.id, project.id, order.session_order ?? [])) out.push({ session, machineId: machine.id });
    }
  }
  return out;
}

/**
 * The order list after dragging the row `key` so it sits before the row now
 * at `toIndex` in `shown` (the keys of the rows as the sidebar lists them in
 * the dragged row's group; shown.length puts it last). The group's keys are
 * replaced by its full new order, so every row of the group is listed from
 * then on; other groups' keys are kept. A key not in `shown` changes nothing.
 */
export function reorderedKeys(order: readonly string[], shown: readonly string[], key: string, toIndex: number): string[] {
  const from = shown.indexOf(key);
  if (from < 0) return [...order];
  const target = Math.max(0, Math.min(shown.length, Math.round(toIndex)));
  const insertAt = target > from ? target - 1 : target;
  const keys = shown.filter((k) => k !== key);
  keys.splice(insertAt, 0, key);
  const group = new Set(shown);
  return [...order.filter((k) => !group.has(k)), ...keys];
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
