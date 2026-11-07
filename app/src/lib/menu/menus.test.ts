import { describe, expect, it, vi } from "vitest";
import type { TabLayout } from "../backend/TabLayout";
import { pane, split } from "../layout/tree";
import type { SessionInfo } from "../protocol/SessionInfo";
import {
  ALONE_IN_TAB,
  buildPaneMenu,
  buildSessionMenu,
  buildTabMenu,
  ONLY_PANE,
  TAB_FULL,
  type PaneMenuActions,
  type PaneMenuContext,
  type SessionMenuActions,
} from "./menus";
import { isEnabled, type MenuEntry, type MenuSubmenu } from "./types";

const noop = () => {};

function paneActions(over: Partial<PaneMenuActions> = {}): PaneMenuActions {
  return {
    copy: noop,
    paste: noop,
    split: noop,
    toggleZoom: noop,
    swapWith: noop,
    moveToNewTab: noop,
    moveToTab: noop,
    applyPreset: noop,
    equalize: noop,
    rename: noop,
    close: noop,
    ...over,
  };
}

function sessionActions(over: Partial<SessionMenuActions> = {}): SessionMenuActions {
  return { openInSplit: noop, moveToNewTab: noop, rename: noop, restart: noop, kill: noop, ...over };
}

function tab(id: string, ids: string[], extra: Partial<TabLayout> = {}): TabLayout {
  const root = ids.length === 1 ? pane(ids[0]) : split("row", ids.map(pane));
  return { id, name: null, root, focused: ids[0], zoomed: null, ...extra };
}

function context(over: Partial<PaneMenuContext> = {}): PaneMenuContext {
  return {
    sessionId: "A",
    hasSelection: false,
    tab: tab("T1", ["A", "B"]),
    otherTabs: [],
    labelOf: (id) => `label-${id}`,
    tabLabelOf: (t) => `tab-${t.id}`,
    ...over,
  };
}

function entry(entries: MenuEntry[], label: string): MenuEntry {
  const found = entries.find((e) => e.kind !== "separator" && e.label === label);
  if (!found) throw new Error(`no entry ${label}`);
  return found;
}

const labels = (entries: MenuEntry[]) => entries.flatMap((e) => (e.kind === "separator" ? [] : [e.label]));

function session(over: Partial<SessionInfo> = {}): SessionInfo {
  return {
    id: "S",
    project_id: "P1",
    kind: "shell",
    command: "pwsh",
    args: [],
    label: "pwsh",
    created_at: 1,
    state: "running",
    state_changed_at: 1,
    exit_code: null,
    claude_session_id: null,
    attachable: true,
    ...over,
  };
}

describe("buildPaneMenu", () => {
  it("lists the spec 9.4 pane menu items in order", () => {
    expect(labels(buildPaneMenu(context(), paneActions()))).toEqual([
      "Copy",
      "Paste",
      "Split right",
      "Split down",
      "Zoom pane",
      "Swap with",
      "Move to",
      "Layout",
      "Equalize sizes",
      "Rename session…",
      "Close pane",
    ]);
  });

  it("disables splits and full Move-to targets in a full tab with the tab-full tooltip", () => {
    const menu = buildPaneMenu(
      context({ tab: tab("T1", ["A", "B", "C", "D"]), otherTabs: [tab("T2", ["E", "F", "G", "H"]), tab("T3", ["I"])] }),
      paneActions(),
    );
    expect(entry(menu, "Split right")).toMatchObject({ disabledReason: TAB_FULL });
    expect(entry(menu, "Split down")).toMatchObject({ disabledReason: TAB_FULL });
    const moveTo = (entry(menu, "Move to") as MenuSubmenu).items;
    expect(entry(moveTo, "tab-T2")).toMatchObject({ disabledReason: TAB_FULL });
    expect(isEnabled(entry(moveTo, "tab-T3"))).toBe(true);
    expect(TAB_FULL).toBe("Tab is full (4 panes)");
  });

  it("enables Copy only with a selection", () => {
    expect(isEnabled(entry(buildPaneMenu(context(), paneActions()), "Copy"))).toBe(false);
    expect(isEnabled(entry(buildPaneMenu(context({ hasSelection: true }), paneActions()), "Copy"))).toBe(true);
  });

  it("runs split with the direction and kind, and shows the split shortcut", () => {
    const split = vi.fn();
    const right = (entry(buildPaneMenu(context(), paneActions({ split })), "Split right") as MenuSubmenu).items;
    expect(entry(right, "New Claude")).toMatchObject({ shortcut: "Alt+Shift+=" });
    const newShell = entry(right, "New shell");
    if (newShell.kind === "action") newShell.run();
    expect(split).toHaveBeenCalledWith("row", "shell");
  });

  it("offers the other panes by label in Swap with", () => {
    const swapWith = vi.fn();
    const items = (entry(buildPaneMenu(context({ tab: tab("T1", ["A", "B", "C"]) }), paneActions({ swapWith })), "Swap with") as MenuSubmenu).items;
    expect(labels(items)).toEqual(["label-B", "label-C"]);
    const c = entry(items, "label-C");
    if (c.kind === "action") c.run();
    expect(swapWith).toHaveBeenCalledWith("C");
  });

  it("disables pane-arrangement items for the only pane in a tab", () => {
    const menu = buildPaneMenu(context({ tab: tab("T1", ["A"]) }), paneActions());
    for (const label of ["Zoom pane", "Swap with", "Layout", "Equalize sizes"]) {
      expect(entry(menu, label), label).toMatchObject({ disabledReason: ONLY_PANE });
    }
    expect(entry((entry(menu, "Move to") as MenuSubmenu).items, "New tab")).toMatchObject({ disabledReason: ALONE_IN_TAB });
  });

  it("labels the zoom item by the current zoom state", () => {
    const zoomed = context({ tab: tab("T1", ["A", "B"], { zoomed: "A" }) });
    expect(labels(buildPaneMenu(zoomed, paneActions()))).toContain("Unzoom pane");
  });
});

describe("buildTabMenu", () => {
  it("offers rename, layout presets and close, with presets disabled for one pane", () => {
    const menu = buildTabMenu(tab("T1", ["A"]), { rename: noop, applyPreset: noop, close: noop });
    expect(labels(menu)).toEqual(["Rename tab…", "Layout", "Close tab"]);
    expect(entry(menu, "Layout")).toMatchObject({ disabledReason: ONLY_PANE });
    const presets = (entry(buildTabMenu(tab("T1", ["A", "B"]), { rename: noop, applyPreset: noop, close: noop }), "Layout") as MenuSubmenu).items;
    expect(labels(presets)).toEqual(["Columns", "Rows", "Grid 2×2", "Main + stack"]);
  });
});

describe("buildSessionMenu", () => {
  const current = tab("T1", ["X"]);

  it("enables Open in split only for another session of the current tab's project in a tab with room", () => {
    const ok = buildSessionMenu({ session: session(), currentTab: current, currentProjectId: "P1", sessionTab: tab("T2", ["S"]) }, sessionActions());
    expect(isEnabled(entry(ok, "Open in split right"))).toBe(true);
    const reasons = (ctx: Parameters<typeof buildSessionMenu>[0]) => {
      const e = entry(buildSessionMenu(ctx, sessionActions()), "Open in split down");
      return e.kind === "action" ? e.disabledReason : undefined;
    };
    expect(reasons({ session: session(), currentTab: null, currentProjectId: null, sessionTab: null })).toBe("No tab is open");
    expect(reasons({ session: session(), currentTab: current, currentProjectId: "P2", sessionTab: null })).toBe("The current tab belongs to another project");
    expect(reasons({ session: session(), currentTab: tab("T1", ["X", "S"]), currentProjectId: "P1", sessionTab: null })).toBe("Already in the current tab");
    expect(reasons({ session: session(), currentTab: tab("T1", ["W", "X", "Y", "Z"]), currentProjectId: "P1", sessionTab: null })).toBe(TAB_FULL);
  });

  it("offers Restart only for exited sessions and disables Kill for them", () => {
    const running = buildSessionMenu({ session: session(), currentTab: current, currentProjectId: "P1", sessionTab: tab("T2", ["S"]) }, sessionActions());
    expect(labels(running)).toEqual(["Open in split right", "Open in split down", "Move to new tab", "Rename…", "Kill"]);
    expect(entry(running, "Move to new tab")).toMatchObject({ disabledReason: ALONE_IN_TAB });
    const exited = buildSessionMenu(
      { session: session({ state: "exited" }), currentTab: current, currentProjectId: "P1", sessionTab: tab("T2", ["S", "Q"]) },
      sessionActions(),
    );
    expect(labels(exited)).toContain("Restart");
    expect(entry(exited, "Kill")).toMatchObject({ disabledReason: "Session has already exited" });
    expect(isEnabled(entry(exited, "Move to new tab"))).toBe(true);
  });
});
