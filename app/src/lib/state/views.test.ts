import { describe, expect, it } from "vitest";
import { applyControl, emptyModel } from "./reducer";
import { orderedMachines, orderedProjects, orderedProjectSessions, orderedSessions, reorderedKeys, reorderedProjectOrder, sessionsForProject, sortedProjects } from "./views";

const base = { kind: "shell", command: "c", args: [] as string[], label: "l", state: "running", state_changed_at: 0, exit_code: null, claude_session_id: null, state_detail: null, renamed: false, prompt_label: null, context_tokens: null, context_window: null, attachable: true } as const;

describe("views", () => {
  it("orders sessions by creation and projects by name", () => {
    let m = applyControl(emptyModel(), {
      type: "Projects",
      projects: [
        { id: "B", path: "b", name: "beta" },
        { id: "A", path: "a", name: "alpha" },
      ],
    });
    m = applyControl(m, {
      type: "Sessions",
      sessions: [
        { ...base, id: "S2", project_id: "A", created_at: 20 },
        { ...base, id: "S1", project_id: "A", created_at: 10 },
        { ...base, id: "S3", project_id: "B", created_at: 5 },
      ],
    });
    expect(sessionsForProject(m, "A").map((s) => s.id)).toEqual(["S1", "S2"]);
    expect(sortedProjects(m).map((p) => p.name)).toEqual(["alpha", "beta"]);
  });

  it("orders sessions by machine, then project name, then creation", () => {
    const local = applyControl(applyControl(emptyModel(), { type: "Projects", projects: [{ id: "B", path: "b", name: "beta" }, { id: "A", path: "a", name: "alpha" }] }), {
      type: "Sessions",
      sessions: [
        { ...base, id: "S3", project_id: "B", created_at: 5 },
        { ...base, id: "S2", project_id: "A", created_at: 20 },
        { ...base, id: "S1", project_id: "A", created_at: 10 },
      ],
    });
    const remote = applyControl(applyControl(emptyModel(), { type: "Projects", projects: [{ id: "R", path: "/r", name: "aaa" }] }), {
      type: "Sessions",
      sessions: [{ ...base, id: "S9", project_id: "R", created_at: 1 }],
    });
    const ordered = orderedSessions([{ id: "local" }, { id: "srv" }], { local, srv: remote });
    expect(ordered.map((o) => `${o.machineId}:${o.session.id}`)).toEqual(["local:S1", "local:S2", "local:S3", "srv:S9"]);
  });
});

describe("project order", () => {
  const model = applyControl(emptyModel(), {
    type: "Projects",
    projects: [
      { id: "A", path: "a", name: "alpha" },
      { id: "B", path: "b", name: "beta" },
      { id: "C", path: "c", name: "gamma" },
    ],
  });
  const names = (order: string[]) => orderedProjects(model, "local", order).map((p) => p.name);

  it("lists ordered projects first, then the rest by name", () => {
    expect(names([])).toEqual(["alpha", "beta", "gamma"]);
    expect(names(["local:C", "local:A"])).toEqual(["gamma", "alpha", "beta"]);
    // Keys of another machine or of removed projects are ignored.
    expect(names(["m2:A", "local:Z", "local:B"])).toEqual(["beta", "alpha", "gamma"]);
  });

  it("moves a dragged project and keeps other machines' keys", () => {
    const order = reorderedProjectOrder(["m2:X"], "local", ["A", "B", "C"], "C", 0);
    expect(order).toEqual(["m2:X", "local:C", "local:A", "local:B"]);
    expect(reorderedProjectOrder(order, "local", ["C", "A", "B"], "C", 3)).toEqual(["m2:X", "local:A", "local:B", "local:C"]);
    expect(reorderedProjectOrder(order, "local", ["C", "A", "B"], "Q", 0)).toEqual(order);
  });
});

describe("location and session order", () => {
  const model = applyControl(applyControl(emptyModel(), { type: "Projects", projects: [{ id: "A", path: "a", name: "alpha" }, { id: "B", path: "b", name: "beta" }] }), {
    type: "Sessions",
    sessions: [
      { ...base, id: "S1", project_id: "A", created_at: 10 },
      { ...base, id: "S2", project_id: "A", created_at: 20 },
      { ...base, id: "S3", project_id: "A", created_at: 30 },
      { ...base, id: "S4", project_id: "B", created_at: 5 },
    ],
  });

  it("lists dragged locations first, then the rest in machines.toml order", () => {
    const machines = [{ id: "local" }, { id: "m1" }, { id: "m2" }, { id: "m3" }];
    expect(orderedMachines(machines, []).map((m) => m.id)).toEqual(["local", "m1", "m2", "m3"]);
    expect(orderedMachines(machines, ["m2", "local", "gone"]).map((m) => m.id)).toEqual(["m2", "local", "m1", "m3"]);
  });

  it("lists dragged sessions first, then the rest by creation time", () => {
    const ids = (order: string[]) => orderedProjectSessions(model, "local", "A", order).map((s) => s.id);
    expect(ids([])).toEqual(["S1", "S2", "S3"]);
    expect(ids(["local:S3", "local:S1"])).toEqual(["S3", "S1", "S2"]);
    // Another machine's key for the same session id does not apply here.
    expect(ids(["m2:S3"])).toEqual(["S1", "S2", "S3"]);
  });

  it("follows every dragged order when listing all sessions", () => {
    const remote = applyControl(applyControl(emptyModel(), { type: "Projects", projects: [{ id: "R", path: "/r", name: "r" }] }), {
      type: "Sessions",
      sessions: [{ ...base, id: "S9", project_id: "R", created_at: 1 }],
    });
    const ordered = orderedSessions([{ id: "local" }, { id: "srv" }], { local: model, srv: remote }, {
      machine_order: ["srv"],
      project_order: ["local:B"],
      session_order: ["local:S2"],
    });
    expect(ordered.map((o) => `${o.machineId}:${o.session.id}`)).toEqual(["srv:S9", "local:S4", "local:S2", "local:S1", "local:S3"]);
  });

  it("moves a row within its group and keeps other groups' keys", () => {
    const shown = ["local:S1", "local:S2", "local:S3"];
    // Drag S3 to the top of project A; project B's key stays.
    const order = reorderedKeys(["local:S4"], shown, "local:S3", 0);
    expect(order).toEqual(["local:S4", "local:S3", "local:S1", "local:S2"]);
    // Drag S3 back to the end (drop position 3 of 3 rows).
    expect(reorderedKeys(order, ["local:S3", "local:S1", "local:S2"], "local:S3", 3)).toEqual(["local:S4", "local:S1", "local:S2", "local:S3"]);
    // Locations use the same function with plain ids.
    expect(reorderedKeys([], ["local", "m1", "m2"], "m2", 0)).toEqual(["m2", "local", "m1"]);
    expect(reorderedKeys(order, shown, "local:S9", 0)).toEqual(order);
  });
});
