import { describe, expect, it } from "vitest";
import { applyControl, emptyModel } from "./reducer";
import { nextActiveAfterRemoval, sessionsForProject, sortedProjects } from "./views";

const base = { kind: "shell", command: "c", args: [] as string[], label: "l", state: "running", state_changed_at: 0, exit_code: null, claude_session_id: null, attachable: true } as const;

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

  describe("nextActiveAfterRemoval", () => {
    const m = applyControl(emptyModel(), {
      type: "Sessions",
      sessions: [
        { ...base, id: "S1", project_id: "A", created_at: 10 },
        { ...base, id: "S2", project_id: "A", created_at: 20 },
        { ...base, id: "S3", project_id: "A", created_at: 30 },
      ],
    });

    it("activates the next session in creation order when a middle one is removed", () => {
      expect(nextActiveAfterRemoval(m, "S2")).toBe("S3");
    });

    it("activates the previous session when the last one is removed", () => {
      expect(nextActiveAfterRemoval(m, "S3")).toBe("S2");
    });

    it("returns null when the project's only session is removed", () => {
      const solo = applyControl(emptyModel(), { type: "Sessions", sessions: [{ ...base, id: "S1", project_id: "A", created_at: 10 }] });
      expect(nextActiveAfterRemoval(solo, "S1")).toBeNull();
    });

    it("returns null for an id that is not in the model", () => {
      expect(nextActiveAfterRemoval(m, "NOPE")).toBeNull();
    });
  });
});
