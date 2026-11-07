import { describe, expect, it } from "vitest";
import { applyControl, emptyModel } from "./reducer";
import { sessionsForProject, sortedProjects } from "./views";

describe("views", () => {
  it("orders sessions by creation and projects by name", () => {
    const base = { kind: "shell", command: "c", args: [] as string[], label: "l", state: "running", state_changed_at: 0, exit_code: null, claude_session_id: null, attachable: true } as const;
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
});
