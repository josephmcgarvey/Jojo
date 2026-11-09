import { describe, expect, it } from "vitest";
import { applyControl, emptyModel } from "./reducer";
import { orderedSessions, sessionsForProject, sortedProjects } from "./views";

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
