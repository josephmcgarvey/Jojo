import { describe, expect, it } from "vitest";
import type { TabLayout } from "../backend/TabLayout";
import { layoutKey, LOCAL_MACHINE_ID } from "../layout/reconcile";
import { newTab } from "../layout/tabs";
import { pane, paneIds, split } from "../layout/tree";
import type { SessionInfo } from "../protocol/SessionInfo";
import { createFailed, emptyWorkspace, projectOf, requestCreate, sessionCreated, sessionRemoved, withProject, type Workspace } from "./workspace";

function session(id: string, project_id = "P1"): SessionInfo {
  return {
    id,
    project_id,
    kind: "shell",
    command: "pwsh.exe",
    args: [],
    label: id.toLowerCase(),
    created_at: 1,
    state: "running",
    state_changed_at: 1,
    exit_code: null,
    claude_session_id: null,
    attachable: true,
  };
}

function ids(): () => string {
  let n = 0;
  return () => `T${++n}`;
}

const KEY = layoutKey(LOCAL_MACHINE_ID, "P1");

function withTabs(ws: Workspace, tabs: TabLayout[]): Workspace {
  return { ...ws, layout: { ...ws.layout, projects: { [KEY]: { tabs, active_tab: tabs[0].id } } } };
}

describe("sessionCreated", () => {
  it("places a requested session in a new active tab and clears the pending entry", () => {
    const ws = requestCreate(emptyWorkspace(), "A", { projectId: "P1", placement: { kind: "tab" } });
    const result = sessionCreated(ws, session("A"), ids());
    expect(result.requested).toBe(true);
    expect(result.closeSessionId).toBeNull();
    expect(result.workspace.pending).toEqual({});
    expect(projectOf(result.workspace.layout, "P1")).toEqual({ tabs: [newTab("T1", "A")], active_tab: "T1" });
  });

  it("places both of two quick creates, whichever reply arrives first", () => {
    let ws = requestCreate(emptyWorkspace(), "A", { projectId: "P1", placement: { kind: "tab" } });
    ws = requestCreate(ws, "B", { projectId: "P1", placement: { kind: "tab" } });
    const newIds = ids();
    const first = sessionCreated(ws, session("B"), newIds);
    const second = sessionCreated(first.workspace, session("A"), newIds);
    expect(first.requested && second.requested).toBe(true);
    const project = projectOf(second.workspace.layout, "P1");
    expect(project.tabs.map((t) => paneIds(t.root))).toEqual([["B"], ["A"]]);
    expect(project.active_tab).toBe("T2");
    expect(second.workspace.pending).toEqual({});
  });

  it("splits beside the requested pane, and falls back to a new tab when the tab filled up meanwhile", () => {
    const base = withTabs(emptyWorkspace(), [newTab("T0", "X")]);
    const ws = requestCreate(base, "N", { projectId: "P1", placement: { kind: "split", tabId: "T0", besideId: "X", direction: "row" } });
    const placed = sessionCreated(ws, session("N"), ids());
    expect(projectOf(placed.workspace.layout, "P1").tabs[0].root).toEqual(split("row", [pane("X"), pane("N")]));

    const full = { ...newTab("T0", "X"), root: split("row", ["X", "Y", "Z", "W"].map(pane)) };
    const fullWs = requestCreate(withTabs(emptyWorkspace(), [full]), "N", {
      projectId: "P1",
      placement: { kind: "split", tabId: "T0", besideId: "X", direction: "row" },
    });
    const fallback = sessionCreated(fullWs, session("N"), ids());
    const project = projectOf(fallback.workspace.layout, "P1");
    expect(project.tabs.map((t) => paneIds(t.root))).toEqual([["X", "Y", "Z", "W"], ["N"]]);
    expect(project.active_tab).toBe("T1");
  });

  it("replaces the old session in place for a restart and reports the old id to close", () => {
    const base = withTabs(emptyWorkspace(), [{ ...newTab("T0", "X"), root: split("row", [pane("OLD"), pane("X")]), focused: "X" }]);
    const ws = requestCreate(base, "NEW", { projectId: "P1", placement: { kind: "replace", oldId: "OLD" } });
    const result = sessionCreated(ws, session("NEW"), ids());
    expect(result.closeSessionId).toBe("OLD");
    expect(paneIds(projectOf(result.workspace.layout, "P1").tabs[0].root)).toEqual(["NEW", "X"]);
  });

  it("adds a session nobody requested in a new tab without activating it", () => {
    const base = withTabs(emptyWorkspace(), [newTab("T0", "X")]);
    const result = sessionCreated(base, session("S"), ids());
    expect(result.requested).toBe(false);
    const project = projectOf(result.workspace.layout, "P1");
    expect(project.tabs.map((t) => t.id)).toEqual(["T0", "T1"]);
    expect(project.active_tab).toBe("T0");
  });

  it("leaves the layout unchanged for a session that is already placed", () => {
    const base = withTabs(emptyWorkspace(), [newTab("T0", "X")]);
    expect(sessionCreated(base, session("X"), ids()).workspace.layout).toBe(base.layout);
  });
});

describe("pending creates and removal", () => {
  it("forgets a pending create that failed", () => {
    const ws = requestCreate(emptyWorkspace(), "A", { projectId: "P1", placement: { kind: "tab" } });
    expect(createFailed(ws, "A").pending).toEqual({});
    const untouched = emptyWorkspace();
    expect(createFailed(untouched, "Z")).toBe(untouched);
  });

  it("removes a session's pane from whichever project holds it", () => {
    const base = withTabs(emptyWorkspace(), [newTab("T0", "X"), newTab("T1", "Y")]);
    expect(projectOf(sessionRemoved(base, "Y").layout, "P1").tabs.map((t) => t.id)).toEqual(["T0"]);
    expect(sessionRemoved(base, "Z")).toBe(base);
  });

  it("returns the same layout when an update changes nothing, and null when it refuses", () => {
    const base = withTabs(emptyWorkspace(), [newTab("T0", "X")]);
    expect(withProject(base.layout, "P1", (p) => p)).toBe(base.layout);
    expect(withProject(base.layout, "P1", () => null)).toBeNull();
    expect(withProject(base.layout, "P2", (p) => p)).toBe(base.layout);
  });
});
