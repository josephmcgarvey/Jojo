import { describe, expect, it } from "vitest";
import { applyControl, emptyModel } from "./reducer";
import { orderedProjects, orderedSessions, reorderedProjectOrder, sessionsForProject, sortedProjects } from "./views";

const base = { kind: "shell", command: "c", args: [] as string[], label: "l", state: "running", state_changed_at: 0, exit_code: null, claude_session_id: null, state_detail: null, renamed: false, prompt_label: null, attachable: true } as const;

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
