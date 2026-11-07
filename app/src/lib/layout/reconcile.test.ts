import { describe, expect, it } from "vitest";
import type { LayoutFile } from "../backend/LayoutFile";
import type { ProjectLayout } from "../backend/ProjectLayout";
import type { SessionInfo } from "../protocol/SessionInfo";
import { applyControl, emptyModel } from "../state/reducer";
import { layoutKey, LOCAL_MACHINE_ID, reconcileAll, reconcileProject } from "./reconcile";
import { newTab } from "./tabs";
import { pane, paneIds, split } from "./tree";

function ids(): () => string {
  let n = 0;
  return () => `NEW${++n}`;
}

describe("reconcileProject", () => {
  it("opens one tab per session, in the given order, when nothing is stored", () => {
    expect(reconcileProject(undefined, ["A", "B"], ids())).toEqual({
      tabs: [newTab("NEW1", "A"), newTab("NEW2", "B")],
      active_tab: "NEW1",
    });
  });

  it("drops session ids the daemon no longer has and removes emptied tabs", () => {
    const stored: ProjectLayout = {
      tabs: [
        { id: "T1", name: null, root: split("row", [pane("A"), pane("GONE")]), focused: "GONE", zoomed: "GONE" },
        newTab("T2", "GONE2"),
      ],
      active_tab: "T2",
    };
    expect(reconcileProject(stored, ["A"], ids())).toEqual({
      tabs: [{ id: "T1", name: null, root: pane("A"), focused: "A", zoomed: null }],
      active_tab: "T1",
    });
  });

  it("places daemon sessions missing from the layout in new tabs after the stored ones", () => {
    const stored: ProjectLayout = { tabs: [newTab("T1", "A")], active_tab: "T1" };
    const result = reconcileProject(stored, ["A", "B", "C"], ids());
    expect(result.tabs.map((t) => [t.id, paneIds(t.root)])).toEqual([
      ["T1", ["A"]],
      ["NEW1", ["B"]],
      ["NEW2", ["C"]],
    ]);
    expect(result.active_tab).toBe("T1");
  });

  it("keeps a session that appears in two tabs only in the first", () => {
    const stored: ProjectLayout = {
      tabs: [newTab("T1", "A"), { id: "T2", name: null, root: split("row", [pane("A"), pane("B")]), focused: "A", zoomed: null }],
      active_tab: "T2",
    };
    const result = reconcileProject(stored, ["A", "B"], ids());
    expect(result.tabs.map((t) => paneIds(t.root))).toEqual([["A"], ["B"]]);
    expect(result.tabs[1].focused).toBe("B");
  });

  it("moves panes beyond the fourth into their own tabs", () => {
    const five = split("row", ["A", "B", "C", "D", "E"].map(pane));
    const stored: ProjectLayout = { tabs: [{ id: "T1", name: null, root: five, focused: "E", zoomed: null }], active_tab: "T1" };
    const result = reconcileProject(stored, ["A", "B", "C", "D", "E"], ids());
    expect(result.tabs.map((t) => paneIds(t.root))).toEqual([["A", "B", "C", "D"], ["E"]]);
    expect(result.tabs[0].focused).toBe("A");
  });

  it("stores a blank name as null and renames duplicate tab ids", () => {
    const stored: ProjectLayout = {
      tabs: [
        { ...newTab("T1", "A"), name: "  " },
        { ...newTab("T1", "B"), name: "logs" },
      ],
      active_tab: "T1",
    };
    const result = reconcileProject(stored, ["A", "B"], ids());
    expect(result.tabs.map((t) => [t.id, t.name])).toEqual([
      ["T1", null],
      ["NEW1", "logs"],
    ]);
  });

  it("gives a project without sessions no tabs and no active tab", () => {
    expect(reconcileProject({ tabs: [newTab("T1", "GONE")], active_tab: "T1" }, [], ids())).toEqual({ tabs: [], active_tab: null });
  });
});

describe("reconcileAll", () => {
  const base = {
    kind: "shell",
    command: "pwsh.exe",
    args: [] as string[],
    label: "pwsh",
    state: "running",
    state_changed_at: 0,
    exit_code: null,
    claude_session_id: null,
    attachable: true,
  } as const;
  const session = (id: string, project_id: string, created_at: number): SessionInfo => ({ ...base, id, project_id, created_at });

  it("reconciles every local project, prunes removed projects and keeps other machines' entries", () => {
    let model = applyControl(emptyModel(), { type: "Projects", projects: [{ id: "P1", path: "C:\\a", name: "a" }] });
    model = applyControl(model, { type: "Sessions", sessions: [session("S2", "P1", 20), session("S1", "P1", 10)] });
    const remote: ProjectLayout = { tabs: [newTab("R1", "X")], active_tab: "R1" };
    const file: LayoutFile = {
      version: 1,
      projects: {
        [layoutKey(LOCAL_MACHINE_ID, "OLD")]: { tabs: [newTab("T1", "Z")], active_tab: "T1" },
        "server1:P9": remote,
      },
    };
    const result = reconcileAll(file, model, ids());
    expect(Object.keys(result.projects).sort()).toEqual(["local:P1", "server1:P9"]);
    expect(result.projects["server1:P9"]).toBe(remote);
    expect(result.projects["local:P1"]!.tabs.map((t) => paneIds(t.root))).toEqual([["S1"], ["S2"]]);
    expect(result.version).toBe(1);
  });
});
