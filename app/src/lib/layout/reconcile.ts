// Brings layout.json in line with the daemon's sessions (spec section 4,
// "Layout" row): session ids that no longer exist are dropped when the
// layout is loaded, and daemon sessions missing from the layout are each
// placed in a new tab.
import type { LayoutFile } from "../backend/LayoutFile";
import type { ProjectLayout } from "../backend/ProjectLayout";
import type { TabLayout } from "../backend/TabLayout";
import type { Model } from "../state/reducer";
import { pruneFlags } from "../state/flags";
import { sessionsForProject } from "../state/views";
import { newTab } from "./tabs";
import { filterPanes, MAX_PANES, paneIds, removePane } from "./tree";

/** Machine id of This PC in layout keys. Plan 5 adds remote machines with their own ids. */
export const LOCAL_MACHINE_ID = "local";

/** A project id is unique only within one daemon, so layout keys include the machine. */
export function layoutKey(machineId: string, projectId: string): string {
  return `${machineId}:${projectId}`;
}

/**
 * Reconciles one project's stored layout with `sessionIds`, the daemon's
 * sessions for that project in creation order:
 *
 * 1. Panes whose session is not in `sessionIds` are removed; a session that
 *    appears more than once keeps only its first pane (tabs in order, tree
 *    order within a tab). Splits collapse and sizes are renormalized.
 * 2. A tab with more than MAX_PANES panes (only possible in a hand-edited
 *    file) keeps its first four; the rest are placed by rule 5.
 * 3. A tab left with no panes is removed. A duplicate tab id gets a new id.
 * 4. A `focused` that names a removed pane moves to the tab's first pane; a
 *    `zoomed` that names a removed pane, or a zoom in a tab with one pane,
 *    is cleared. A blank name becomes null.
 * 5. Every session not placed by rules 1-2 gets its own new tab, appended in
 *    creation order.
 * 6. `active_tab` is kept when it names a remaining tab; otherwise the first
 *    tab becomes active (null when there are no tabs).
 */
export function reconcileProject(stored: ProjectLayout | undefined, sessionIds: string[], newTabId: () => string): ProjectLayout {
  const live = new Set(sessionIds);
  const placed = new Set<string>();
  const tabIds = new Set<string>();
  const tabs: TabLayout[] = [];
  for (const tab of stored?.tabs ?? []) {
    let root = filterPanes(tab.root, (id) => {
      if (!live.has(id) || placed.has(id)) return false;
      placed.add(id);
      return true;
    });
    if (!root) continue;
    for (const extra of paneIds(root).slice(MAX_PANES)) {
      root = removePane(root, extra) ?? root;
      placed.delete(extra);
    }
    const kept = paneIds(root);
    const id = tabIds.has(tab.id) ? newTabId() : tab.id;
    tabIds.add(id);
    tabs.push({
      id,
      name: tab.name !== null && tab.name.trim() !== "" ? tab.name : null,
      root,
      focused: kept.includes(tab.focused) ? tab.focused : kept[0],
      zoomed: tab.zoomed !== null && kept.length > 1 && kept.includes(tab.zoomed) ? tab.zoomed : null,
    });
  }
  for (const sessionId of sessionIds) {
    if (placed.has(sessionId)) continue;
    const id = newTabId();
    tabIds.add(id);
    tabs.push(newTab(id, sessionId));
    placed.add(sessionId);
  }
  const storedActive = stored?.active_tab ?? null;
  const active_tab = storedActive !== null && tabs.some((t) => t.id === storedActive) ? storedActive : (tabs[0]?.id ?? null);
  return { tabs, active_tab };
}

/**
 * Reconciles every project of the local daemon. Entries for local projects
 * the daemon no longer has are dropped; entries for other machines are kept
 * unchanged (Plan 5 reconciles them against their own daemons).
 */
export function reconcileAll(file: LayoutFile, model: Model, newTabId: () => string): LayoutFile {
  return reconcileMachine(file, LOCAL_MACHINE_ID, model, newTabId);
}

/**
 * Reconciles the layouts of one machine's projects with that machine's model
 * (spec 4): every `<machineId>:` key is rebuilt from `model`, and the keys of
 * other machines are kept unchanged, so a machine that is not connected yet
 * keeps its stored tabs.
 */
export function reconcileMachine(file: LayoutFile, machineId: string, model: Model, newTabId: () => string): LayoutFile {
  const prefix = `${machineId}:`;
  const projects: Record<string, ProjectLayout> = {};
  for (const [key, value] of Object.entries(file.projects)) {
    if (value && !key.startsWith(prefix)) projects[key] = value;
  }
  for (const project of Object.values(model.projects)) {
    const key = layoutKey(machineId, project.id);
    const sessionIds = sessionsForProject(model, project.id).map((s) => s.id);
    projects[key] = reconcileProject(file.projects[key], sessionIds, newTabId);
  }
  // The sidebar order keeps other machines' keys and this machine's keys
  // whose project still exists.
  const project_order = (file.project_order ?? []).filter((key) => !key.startsWith(prefix) || key in projects);
  // Flags of this machine's sessions that are gone are dropped.
  const flagged = pruneFlags(file.flagged ?? [], machineId, Object.keys(model.sessions));
  // So is the sidebar position of a removed session (the same key shape as a flag).
  const session_order = pruneFlags(file.session_order ?? [], machineId, Object.keys(model.sessions));
  return { version: file.version, projects, project_order, machine_order: file.machine_order ?? [], session_order, collapsed: file.collapsed ?? [], flagged };
}
