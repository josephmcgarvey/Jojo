import { describe, expect, it } from "vitest";
import type { TabLayout } from "../backend/TabLayout";
import {
  activeTab,
  addTab,
  cycleTab,
  emptyProject,
  equalizeTab,
  focusSession,
  moveBeside,
  moveToNewTab,
  moveToTab,
  newTab,
  presetTab,
  removeSession,
  renameTab,
  replaceSession,
  selectTab,
  setTabRoot,
  splitBeside,
  swapInTab,
  tabLabel,
  tabOf,
  toggleZoom,
  type ProjectLayout,
} from "./tabs";
import { pane, paneIds, split } from "./tree";

function tab(id: string, root: TabLayout["root"], focused: string, extra: Partial<TabLayout> = {}): TabLayout {
  return { id, name: null, root, focused, zoomed: null, ...extra };
}

function project(...tabs: TabLayout[]): ProjectLayout {
  return { tabs, active_tab: tabs[0]?.id ?? null };
}

const pair = () => tab("T1", split("row", [pane("A"), pane("B")]), "A");

describe("adding, selecting and cycling tabs", () => {
  it("activates a new tab only when asked, unless nothing is active", () => {
    const one = addTab(emptyProject(), newTab("T1", "A"), false);
    expect(one.active_tab).toBe("T1");
    const two = addTab(one, newTab("T2", "B"), false);
    expect(two.active_tab).toBe("T1");
    expect(addTab(one, newTab("T2", "B"), true).active_tab).toBe("T2");
  });

  it("falls back to the first tab when the active id is missing", () => {
    const p = { tabs: [newTab("T1", "A")], active_tab: "GONE" };
    expect(activeTab(p)?.id).toBe("T1");
    expect(activeTab(emptyProject())).toBeNull();
  });

  it("returns the same object when selecting the active or an unknown tab", () => {
    const p = project(newTab("T1", "A"), newTab("T2", "B"));
    expect(selectTab(p, "T1")).toBe(p);
    expect(selectTab(p, "NOPE")).toBe(p);
    expect(selectTab(p, "T2").active_tab).toBe("T2");
  });

  it("cycles through tabs in both directions, wrapping around", () => {
    const p = project(newTab("T1", "A"), newTab("T2", "B"), newTab("T3", "C"));
    expect(cycleTab(p, 1).active_tab).toBe("T2");
    expect(cycleTab(p, -1).active_tab).toBe("T3");
    const single = project(newTab("T1", "A"));
    expect(cycleTab(single, 1)).toBe(single);
  });
});

describe("renameTab and tabLabel", () => {
  it("trims the name and stores a blank name as null", () => {
    const p = project(newTab("T1", "A"));
    expect(renameTab(p, "T1", "  build  ").tabs[0].name).toBe("build");
    const named = renameTab(p, "T1", "build");
    expect(renameTab(named, "T1", "   ").tabs[0].name).toBeNull();
    expect(renameTab(named, "T1", "build")).toBe(named);
  });

  it("labels a tab by its name, else its focused session's label, else Tab", () => {
    const labels: Record<string, string> = { A: "claude" };
    expect(tabLabel(tab("T1", pane("A"), "A", { name: "work" }), (id) => labels[id])).toBe("work");
    expect(tabLabel(tab("T1", pane("A"), "A"), (id) => labels[id])).toBe("claude");
    expect(tabLabel(tab("T1", pane("X"), "X"), (id) => labels[id])).toBe("Tab");
  });
});

describe("focusSession", () => {
  it("activates the session's tab and focuses its pane", () => {
    const p = project(newTab("T1", "X"), { ...pair(), id: "T2" });
    const next = focusSession(p, "B");
    expect(next.active_tab).toBe("T2");
    expect(tabOf(next, "B")?.focused).toBe("B");
  });

  it("clears a zoom on another pane so the focused pane is visible", () => {
    const p = project({ ...pair(), zoomed: "A" });
    expect(focusSession(p, "B").tabs[0].zoomed).toBeNull();
    expect(focusSession(p, "A")).toBe(p);
  });
});

describe("removeSession", () => {
  it("moves focus to the next pane in order, else the previous one", () => {
    const p = project(tab("T1", split("row", [pane("A"), pane("B"), pane("C")]), "B"));
    expect(removeSession(p, "B").tabs[0].focused).toBe("C");
    const last = project(tab("T1", split("row", [pane("A"), pane("B")]), "B"));
    expect(removeSession(last, "B").tabs[0].focused).toBe("A");
  });

  it("closes a tab whose last pane is removed and activates its right neighbor, else its left", () => {
    const p = { tabs: [newTab("T1", "A"), newTab("T2", "B"), newTab("T3", "C")], active_tab: "T2" };
    expect(removeSession(p, "B")).toEqual({ tabs: [newTab("T1", "A"), newTab("T3", "C")], active_tab: "T3" });
    const end = { tabs: [newTab("T1", "A"), newTab("T2", "B")], active_tab: "T2" };
    expect(removeSession(end, "B").active_tab).toBe("T1");
  });

  it("clears the zoom of a removed pane and of a tab left with one pane", () => {
    const p = project({ ...pair(), zoomed: "B" });
    expect(removeSession(p, "B").tabs[0].zoomed).toBeNull();
    const other = project({ ...pair(), zoomed: "A" });
    expect(removeSession(other, "B").tabs[0].zoomed).toBeNull();
  });

  it("returns the same object for a session in no tab", () => {
    const p = project(pair());
    expect(removeSession(p, "Z")).toBe(p);
  });
});

describe("splitBeside and toggleZoom", () => {
  it("adds the pane, focuses it, activates the tab and clears the zoom", () => {
    const p = { tabs: [newTab("T0", "X"), { ...pair(), zoomed: "A" }], active_tab: "T0" };
    const next = splitBeside(p, "T1", "A", "N", "column")!;
    expect(next.active_tab).toBe("T1");
    const t = tabOf(next, "N")!;
    expect(paneIds(t.root)).toEqual(["A", "N", "B"]);
    expect(t.focused).toBe("N");
    expect(t.zoomed).toBeNull();
  });

  it("returns null for a full tab or an unknown tab", () => {
    const full = project(tab("T1", split("row", [pane("A"), pane("B"), pane("C"), pane("D")]), "A"));
    expect(splitBeside(full, "T1", "A", "N", "row")).toBeNull();
    expect(splitBeside(project(pair()), "NOPE", "A", "N", "row")).toBeNull();
  });

  it("toggles zoom on a pane and focuses it; a single pane never zooms", () => {
    const zoomed = toggleZoom(project(pair()), "B");
    expect(zoomed.tabs[0]).toMatchObject({ zoomed: "B", focused: "B" });
    expect(toggleZoom(zoomed, "B").tabs[0].zoomed).toBeNull();
    const single = project(newTab("T1", "A"));
    expect(toggleZoom(single, "A")).toBe(single);
  });
});

describe("moving panes between tabs", () => {
  it("moves a pane to a new active tab, and refuses when it is alone in its tab", () => {
    const next = moveToNewTab(project(pair()), "B", "T9")!;
    expect(next.tabs.map((t) => paneIds(t.root))).toEqual([["A"], ["B"]]);
    expect(next.active_tab).toBe("T9");
    expect(moveToNewTab(project(newTab("T1", "A")), "A", "T9")).toBeNull();
  });

  it("moves a pane beside the target tab's focused pane and closes the emptied source tab", () => {
    const p = project(newTab("T1", "A"), tab("T2", split("column", [pane("B"), pane("C")]), "C"));
    const next = moveToTab(p, "A", "T2")!;
    expect(next.tabs).toHaveLength(1);
    expect(next.tabs[0].root).toEqual(split("column", [pane("B"), split("row", [pane("C"), pane("A")])]));
    expect(next.tabs[0].focused).toBe("A");
    expect(next.active_tab).toBe("T2");
  });

  it("refuses a full target, the same tab and an unknown target", () => {
    const full = tab("T2", split("row", [pane("B"), pane("C"), pane("D"), pane("E")]), "B");
    expect(moveToTab(project(newTab("T1", "A"), full), "A", "T2")).toBeNull();
    expect(moveToTab(project(pair()), "A", "T1")).toBeNull();
    expect(moveToTab(project(pair()), "A", "NOPE")).toBeNull();
  });

  it("moves beside a chosen pane in a chosen direction", () => {
    const p = project(newTab("T1", "A"), tab("T2", split("row", [pane("B"), pane("C")]), "C"));
    const next = moveBeside(p, "A", "T2", "B", "column")!;
    expect(next.tabs[0].root).toEqual(split("row", [split("column", [pane("B"), pane("A")]), pane("C")]));
  });
});

describe("swap, presets, equalize, roots and replacement", () => {
  it("swaps two panes of the same tab only", () => {
    const p = project(pair(), newTab("T2", "C"));
    expect(paneIds(swapInTab(p, "A", "B").tabs[0].root)).toEqual(["B", "A"]);
    expect(swapInTab(p, "A", "C")).toBe(p);
  });

  it("applies a preset around the focused pane and clears the zoom", () => {
    const p = project(tab("T1", split("row", [pane("A"), pane("B"), pane("C")]), "B", { zoomed: "B" }));
    const next = presetTab(p, "T1", "main_stack");
    expect(next.tabs[0].root).toEqual(split("row", [pane("B"), split("column", [pane("A"), pane("C")])], [0.6, 0.4]));
    expect(next.tabs[0].zoomed).toBeNull();
  });

  it("equalizes a tab", () => {
    const p = project(tab("T1", split("row", [pane("A"), pane("B")], [0.9, 0.1]), "A"));
    expect(equalizeTab(p, "T1").tabs[0].root).toEqual(split("row", [pane("A"), pane("B")]));
  });

  it("sets a new root only when it holds the same panes", () => {
    const p = project(pair());
    const resized = split("row", [pane("A"), pane("B")], [0.7, 0.3]);
    expect(setTabRoot(p, "T1", resized).tabs[0].root).toEqual(resized);
    expect(setTabRoot(p, "T1", split("row", [pane("A"), pane("Z")]))).toBe(p);
  });

  it("replaces a session in place and focuses the new one", () => {
    const p = { tabs: [newTab("T0", "X"), pair()], active_tab: "T0" };
    const next = replaceSession(p, "A", "N")!;
    expect(paneIds(next.tabs[1].root)).toEqual(["N", "B"]);
    expect(next.tabs[1].focused).toBe("N");
    expect(next.active_tab).toBe("T1");
    expect(replaceSession(p, "Z", "N")).toBeNull();
    expect(replaceSession(p, "A", "B")).toBeNull();
  });
});
