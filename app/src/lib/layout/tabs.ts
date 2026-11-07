// Operations on one project's tabs (spec sections 9.3 and 9.4). Pure:
// every function returns a new ProjectLayout, or the same object when
// nothing changed, so the caller can skip writing layout.json by comparing
// identities. Functions returning `| null` return null when the operation is
// not possible (the caller then disables or ignores the action).
import type { ProjectLayout } from "../backend/ProjectLayout";
import type { TabLayout } from "../backend/TabLayout";
import {
  applyPreset,
  containsPane,
  countPanes,
  equalize,
  MAX_PANES,
  pane,
  paneIds,
  removePane,
  replacePane,
  splitPane,
  swapPanes,
  type LayoutNode,
  type LayoutPreset,
  type SplitDirection,
} from "./tree";

export type { ProjectLayout, TabLayout };

export function emptyProject(): ProjectLayout {
  return { tabs: [], active_tab: null };
}

export function newTab(id: string, sessionId: string): TabLayout {
  return { id, name: null, root: pane(sessionId), focused: sessionId, zoomed: null };
}

/** The active tab; the first tab when `active_tab` is null or names no tab. */
export function activeTab(project: ProjectLayout): TabLayout | null {
  return project.tabs.find((t) => t.id === project.active_tab) ?? project.tabs[0] ?? null;
}

export function tabOf(project: ProjectLayout, sessionId: string): TabLayout | null {
  return project.tabs.find((t) => containsPane(t.root, sessionId)) ?? null;
}

function findTab(project: ProjectLayout, tabId: string): TabLayout | null {
  return project.tabs.find((t) => t.id === tabId) ?? null;
}

function putTab(project: ProjectLayout, tab: TabLayout): ProjectLayout {
  return { ...project, tabs: project.tabs.map((t) => (t.id === tab.id ? tab : t)) };
}

export function addTab(project: ProjectLayout, tab: TabLayout, activate: boolean): ProjectLayout {
  const active_tab = activate || project.active_tab === null ? tab.id : project.active_tab;
  return { tabs: [...project.tabs, tab], active_tab };
}

export function selectTab(project: ProjectLayout, tabId: string): ProjectLayout {
  if (project.active_tab === tabId || !findTab(project, tabId)) return project;
  return { ...project, active_tab: tabId };
}

export function cycleTab(project: ProjectLayout, delta: 1 | -1): ProjectLayout {
  const current = activeTab(project);
  if (!current || project.tabs.length < 2) return project;
  const index = project.tabs.indexOf(current);
  const next = project.tabs[(index + delta + project.tabs.length) % project.tabs.length];
  return selectTab(project, next.id);
}

/** Removes a tab; when it was active, its right neighbor (else its left) becomes active. */
export function closeTab(project: ProjectLayout, tabId: string): ProjectLayout {
  const index = project.tabs.findIndex((t) => t.id === tabId);
  if (index < 0) return project;
  const wasActive = activeTab(project)?.id === tabId;
  const tabs = project.tabs.filter((t) => t.id !== tabId);
  const active_tab = wasActive ? ((tabs[index] ?? tabs[index - 1])?.id ?? null) : project.active_tab;
  return { tabs, active_tab };
}

/** Trims `name`; a blank name is stored as null, which shows the focused session's label. */
export function renameTab(project: ProjectLayout, tabId: string, name: string): ProjectLayout {
  const tab = findTab(project, tabId);
  if (!tab) return project;
  const trimmed = name.trim();
  const next = trimmed === "" ? null : trimmed;
  return tab.name === next ? project : putTab(project, { ...tab, name: next });
}

/**
 * Activates the session's tab and focuses its pane (spec 9.3 rule 5). A zoom
 * on another pane is cleared, because the zoomed pane hides every other one.
 */
export function focusSession(project: ProjectLayout, sessionId: string): ProjectLayout {
  const tab = tabOf(project, sessionId);
  if (!tab) return project;
  const zoomed = tab.zoomed !== null && tab.zoomed !== sessionId ? null : tab.zoomed;
  if (project.active_tab === tab.id && tab.focused === sessionId && zoomed === tab.zoomed) return project;
  return { ...putTab(project, { ...tab, focused: sessionId, zoomed }), active_tab: tab.id };
}

/**
 * Removes a session's pane. Focus moves to the next pane in tree order, or
 * the previous one when it was last. A tab left with no panes is closed.
 */
export function removeSession(project: ProjectLayout, sessionId: string): ProjectLayout {
  const tab = tabOf(project, sessionId);
  if (!tab) return project;
  const root = removePane(tab.root, sessionId);
  if (!root) return closeTab(project, tab.id);
  const remaining = paneIds(root);
  let focused = tab.focused;
  if (focused === sessionId) {
    const order = paneIds(tab.root);
    const index = order.indexOf(sessionId);
    const after = order.slice(index + 1).find((id) => remaining.includes(id));
    const before = order
      .slice(0, index)
      .reverse()
      .find((id) => remaining.includes(id));
    focused = after ?? before ?? remaining[0];
  }
  const zoomed = tab.zoomed === sessionId || remaining.length < 2 ? null : tab.zoomed;
  return putTab(project, { ...tab, root, focused, zoomed });
}

/** Adds `newId` beside `besideId` in tab `tabId`, focuses it and activates the tab. */
export function splitBeside(
  project: ProjectLayout,
  tabId: string,
  besideId: string,
  newId: string,
  direction: SplitDirection,
): ProjectLayout | null {
  const tab = findTab(project, tabId);
  if (!tab) return null;
  const root = splitPane(tab.root, besideId, newId, direction);
  if (!root) return null;
  return { ...putTab(project, { ...tab, root, focused: newId, zoomed: null }), active_tab: tab.id };
}

/** Zooms the session's pane, or unzooms it if it is already zoomed (spec 9.4, Zoom pane). */
export function toggleZoom(project: ProjectLayout, sessionId: string): ProjectLayout {
  const tab = tabOf(project, sessionId);
  if (!tab || countPanes(tab.root) < 2) return project;
  return putTab(project, { ...tab, focused: sessionId, zoomed: tab.zoomed === sessionId ? null : sessionId });
}

/**
 * Moves a session out of its tab and beside `besideId` in another tab of the
 * same project. Returns null when the target is the same tab, is full, or
 * does not hold `besideId`. An emptied source tab is closed.
 */
export function moveBeside(
  project: ProjectLayout,
  sessionId: string,
  targetTabId: string,
  besideId: string,
  direction: SplitDirection,
): ProjectLayout | null {
  const source = tabOf(project, sessionId);
  const target = findTab(project, targetTabId);
  if (!source || !target || source.id === target.id) return null;
  if (countPanes(target.root) >= MAX_PANES || !containsPane(target.root, besideId)) return null;
  return splitBeside(removeSession(project, sessionId), targetTabId, besideId, sessionId, direction);
}

/** Spec 9.4, Move to ▸ other tab: the pane goes to the right of the target's focused pane. */
export function moveToTab(project: ProjectLayout, sessionId: string, targetTabId: string): ProjectLayout | null {
  const target = findTab(project, targetTabId);
  if (!target) return null;
  return moveBeside(project, sessionId, targetTabId, target.focused, "row");
}

/** Spec 9.4, Move to ▸ New tab. Null when the session is the only pane in its tab. */
export function moveToNewTab(project: ProjectLayout, sessionId: string, newTabId: string): ProjectLayout | null {
  const source = tabOf(project, sessionId);
  if (!source || countPanes(source.root) < 2) return null;
  return addTab(removeSession(project, sessionId), newTab(newTabId, sessionId), true);
}

export function swapInTab(project: ProjectLayout, a: string, b: string): ProjectLayout {
  const tab = tabOf(project, a);
  if (!tab || a === b || !containsPane(tab.root, b)) return project;
  return putTab(project, { ...tab, root: swapPanes(tab.root, a, b) });
}

export function presetTab(project: ProjectLayout, tabId: string, preset: LayoutPreset): ProjectLayout {
  const tab = findTab(project, tabId);
  if (!tab || countPanes(tab.root) < 2) return project;
  return putTab(project, { ...tab, root: applyPreset(tab.root, preset, tab.focused), zoomed: null });
}

export function equalizeTab(project: ProjectLayout, tabId: string): ProjectLayout {
  const tab = findTab(project, tabId);
  if (!tab || tab.root.type === "pane") return project;
  return putTab(project, { ...tab, root: equalize(tab.root) });
}

/** Replaces a tab's tree (used by divider drags); refused when the panes differ. */
export function setTabRoot(project: ProjectLayout, tabId: string, root: LayoutNode): ProjectLayout {
  const tab = findTab(project, tabId);
  if (!tab) return project;
  const before = [...paneIds(tab.root)].sort().join(",");
  const after = [...paneIds(root)].sort().join(",");
  return before === after ? putTab(project, { ...tab, root }) : project;
}

/**
 * Puts `newId` in `oldId`'s position (Restart: the new session replaces the
 * old row), focuses it and activates its tab. Null when `oldId` is in no tab
 * or `newId` is already placed.
 */
export function replaceSession(project: ProjectLayout, oldId: string, newId: string): ProjectLayout | null {
  const tab = tabOf(project, oldId);
  if (!tab || tabOf(project, newId)) return null;
  const zoomed = tab.zoomed === oldId ? newId : tab.zoomed;
  const replaced = { ...tab, root: replacePane(tab.root, oldId, newId), focused: newId, zoomed };
  return { ...putTab(project, replaced), active_tab: tab.id };
}

export function tabLabel(tab: TabLayout, labelOf: (sessionId: string) => string | undefined): string {
  return tab.name ?? labelOf(tab.focused) ?? "Tab";
}
