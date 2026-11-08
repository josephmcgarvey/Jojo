// The pane menu, tab menu and sidebar session menu of spec 9.4, built from
// plain data so their enabled states and tooltips are tested without the UI.
import type { TabLayout } from "../backend/TabLayout";
import { SHORTCUT_LABELS } from "../input/shortcuts";
import { countPanes, MAX_PANES, paneIds, type LayoutPreset, type SplitDirection } from "../layout/tree";
import type { SessionInfo } from "../protocol/SessionInfo";
import type { SessionKind } from "../protocol/SessionKind";
import { canRestart, canResume } from "../state/sessionRules";
import { SEPARATOR, type MenuEntry } from "./types";

/** Spec 9.4, "Limits": the tooltip of split items and Move-to targets in a full tab. */
export const TAB_FULL = "Tab is full (4 panes)";
export const ONLY_PANE = "Only pane in this tab";
export const ALONE_IN_TAB = "Already the only pane in its tab";

export const PRESET_LABELS: Record<LayoutPreset, string> = {
  columns: "Columns",
  rows: "Rows",
  grid: "Grid 2×2",
  main_stack: "Main + stack",
};

function presetItems(apply: (preset: LayoutPreset) => void): MenuEntry[] {
  return (Object.keys(PRESET_LABELS) as LayoutPreset[]).map(
    (preset): MenuEntry => ({ kind: "action", label: PRESET_LABELS[preset], run: () => apply(preset) }),
  );
}

export interface PaneMenuContext {
  sessionId: string;
  hasSelection: boolean;
  /** The tab that holds the pane. */
  tab: TabLayout;
  /** The project's other tabs, in order (Move to targets). */
  otherTabs: TabLayout[];
  labelOf(sessionId: string): string;
  tabLabelOf(tab: TabLayout): string;
  /**
   * The shells offered under New shell, default first (Plan 3 amendment B).
   * Empty when detection found none; New shell then uses the defaultShell
   * setting.
   */
  shells: { label: string; program: string }[];
}

export interface PaneMenuActions {
  copy(): void;
  paste(): void;
  /** `shell` is the program for a shell session; omitted, the defaultShell setting applies. */
  split(direction: SplitDirection, kind: SessionKind, shell?: string): void;
  toggleZoom(): void;
  swapWith(sessionId: string): void;
  moveToNewTab(): void;
  moveToTab(tabId: string): void;
  applyPreset(preset: LayoutPreset): void;
  equalize(): void;
  rename(): void;
  close(): void;
}

export function buildPaneMenu(ctx: PaneMenuContext, actions: PaneMenuActions): MenuEntry[] {
  const ids = paneIds(ctx.tab.root);
  const single = ids.length < 2;
  const full = ids.length >= MAX_PANES;
  const others = ids.filter((id) => id !== ctx.sessionId);
  const splitItems = (direction: SplitDirection, shortcut: string): MenuEntry[] => [
    { kind: "action", label: "New Claude", shortcut, run: () => actions.split(direction, "claude") },
    ctx.shells.length === 0
      ? { kind: "action", label: "New shell", run: () => actions.split(direction, "shell") }
      : {
          kind: "submenu",
          label: "New shell",
          items: ctx.shells.map((shell): MenuEntry => ({ kind: "action", label: shell.label, run: () => actions.split(direction, "shell", shell.program) })),
        },
  ];
  return [
    {
      kind: "action",
      label: "Copy",
      shortcut: SHORTCUT_LABELS.copy,
      disabledReason: ctx.hasSelection ? undefined : "No text selected",
      run: actions.copy,
    },
    { kind: "action", label: "Paste", shortcut: SHORTCUT_LABELS.paste, run: actions.paste },
    SEPARATOR,
    { kind: "submenu", label: "Split right", disabledReason: full ? TAB_FULL : undefined, items: splitItems("row", SHORTCUT_LABELS["split-right"]) },
    { kind: "submenu", label: "Split down", disabledReason: full ? TAB_FULL : undefined, items: splitItems("column", SHORTCUT_LABELS["split-down"]) },
    {
      kind: "action",
      label: ctx.tab.zoomed === ctx.sessionId ? "Unzoom pane" : "Zoom pane",
      shortcut: SHORTCUT_LABELS.zoom,
      disabledReason: single ? ONLY_PANE : undefined,
      run: actions.toggleZoom,
    },
    {
      kind: "submenu",
      label: "Swap with",
      disabledReason: single ? ONLY_PANE : undefined,
      items: others.map((id): MenuEntry => ({ kind: "action", label: ctx.labelOf(id), run: () => actions.swapWith(id) })),
    },
    {
      kind: "submenu",
      label: "Move to",
      items: [
        { kind: "action", label: "New tab", disabledReason: single ? ALONE_IN_TAB : undefined, run: actions.moveToNewTab },
        ...ctx.otherTabs.map(
          (t): MenuEntry => ({
            kind: "action",
            label: ctx.tabLabelOf(t),
            disabledReason: countPanes(t.root) >= MAX_PANES ? TAB_FULL : undefined,
            run: () => actions.moveToTab(t.id),
          }),
        ),
      ],
    },
    { kind: "submenu", label: "Layout", disabledReason: single ? ONLY_PANE : undefined, items: presetItems(actions.applyPreset) },
    { kind: "action", label: "Equalize sizes", disabledReason: single ? ONLY_PANE : undefined, run: actions.equalize },
    SEPARATOR,
    { kind: "action", label: "Rename session…", run: actions.rename },
    { kind: "action", label: "Close pane", shortcut: SHORTCUT_LABELS["close-pane"], run: actions.close },
  ];
}

export interface TabMenuActions {
  rename(): void;
  applyPreset(preset: LayoutPreset): void;
  close(): void;
}

export function buildTabMenu(tab: TabLayout, actions: TabMenuActions): MenuEntry[] {
  const single = countPanes(tab.root) < 2;
  return [
    { kind: "action", label: "Rename tab…", run: actions.rename },
    { kind: "submenu", label: "Layout", disabledReason: single ? ONLY_PANE : undefined, items: presetItems(actions.applyPreset) },
    SEPARATOR,
    { kind: "action", label: "Close tab", run: actions.close },
  ];
}

export interface SessionMenuContext {
  session: SessionInfo;
  /** The active tab of the selected project, if any. */
  currentTab: TabLayout | null;
  /** The selected project's id, which owns `currentTab`. */
  currentProjectId: string | null;
  /** The tab that holds this session. */
  sessionTab: TabLayout | null;
}

export interface SessionMenuActions {
  openInSplit(direction: SplitDirection): void;
  moveToNewTab(): void;
  rename(): void;
  restart(): void;
  resume(): void;
  kill(): void;
}

/**
 * Why "Open in split" is disabled, or undefined when it is enabled. Spec
 * 9.4: both items move the session into the current tab next to the
 * focused pane, and are enabled only when the current tab belongs to the
 * session's project.
 */
function openInSplitBlocker(ctx: SessionMenuContext): string | undefined {
  if (!ctx.currentTab) return "No tab is open";
  if (ctx.currentProjectId !== ctx.session.project_id) return "The current tab belongs to another project";
  if (paneIds(ctx.currentTab.root).includes(ctx.session.id)) return "Already in the current tab";
  if (countPanes(ctx.currentTab.root) >= MAX_PANES) return TAB_FULL;
  return undefined;
}

export function buildSessionMenu(ctx: SessionMenuContext, actions: SessionMenuActions): MenuEntry[] {
  const blocked = openInSplitBlocker(ctx);
  const alone = !ctx.sessionTab || countPanes(ctx.sessionTab.root) < 2;
  const entries: MenuEntry[] = [
    { kind: "action", label: "Open in split right", disabledReason: blocked, run: () => actions.openInSplit("row") },
    { kind: "action", label: "Open in split down", disabledReason: blocked, run: () => actions.openInSplit("column") },
    { kind: "action", label: "Move to new tab", disabledReason: alone ? ALONE_IN_TAB : undefined, run: actions.moveToNewTab },
    SEPARATOR,
    { kind: "action", label: "Rename…", run: actions.rename },
  ];
  if (canResume(ctx.session)) entries.push({ kind: "action", label: "Resume", run: actions.resume });
  if (canRestart(ctx.session)) entries.push({ kind: "action", label: "Restart", run: actions.restart });
  entries.push({
    kind: "action",
    label: "Kill",
    disabledReason: ctx.session.state === "exited" ? "Session has already exited" : undefined,
    run: actions.kill,
  });
  return entries;
}
