import { describe, expect, it } from "vitest";
import type { SessionInfo } from "../protocol/SessionInfo";
import { applyControl, clearConnectionErrors, emptyModel } from "./reducer";

const project = { id: "P1", path: "C:\\work\\app", name: "app" };
function session(id: string, over: Partial<SessionInfo> = {}): SessionInfo {
  return {
    id,
    project_id: "P1",
    kind: "shell",
    command: "pwsh.exe",
    args: [],
    label: "pwsh",
    created_at: 1,
    state: "running",
    state_changed_at: 1,
    exit_code: null,
    claude_session_id: null,
    state_detail: null,
    renamed: false,
    prompt_label: null,
    attachable: true,
    ...over,
  };
}

describe("applyControl", () => {
  it("replaces lists and upserts events idempotently", () => {
    let m = applyControl(emptyModel(), { type: "Projects", projects: [project] });
    m = applyControl(m, { type: "Sessions", sessions: [session("S1")] });
    m = applyControl(m, { type: "SessionCreated", session: session("S1") });
    expect(Object.keys(m.sessions)).toEqual(["S1"]);
    m = applyControl(m, { type: "SessionUpdated", session: session("S1", { label: "renamed" }) });
    expect(m.sessions.S1.label).toBe("renamed");
  });

  it("marks exits and ignores events for unknown ids", () => {
    let m = applyControl(emptyModel(), { type: "Sessions", sessions: [session("S1")] });
    m = applyControl(m, { type: "SessionExited", session_id: "S1", exit_code: 0 });
    expect(m.sessions.S1.state).toBe("exited");
    expect(m.sessions.S1.exit_code).toBe(0);
    const unchanged = applyControl(m, { type: "SessionExited", session_id: "NOPE", exit_code: 1 });
    expect(unchanged).toEqual(m);
    m = applyControl(m, { type: "SessionRemoved", session_id: "S1" });
    expect(m.sessions).toEqual({});
  });

  it("tracks projects and the last error", () => {
    let m = applyControl(emptyModel(), { type: "ProjectAdded", project });
    expect(m.projects.P1.name).toBe("app");
    m = applyControl(m, { type: "Error", request: "AddProject", message: "nope", session_id: null });
    expect(m.lastError).toEqual({ request: "AddProject", message: "nope", session_id: null });
    m = applyControl(m, { type: "ProjectRemoved", id: "P1" });
    expect(m.projects).toEqual({});
  });

  it("clears only connection errors on reconnect", () => {
    for (const request of ["Hello", "Events"]) {
      const m = applyControl(emptyModel(), { type: "Error", request, message: "reconnect", session_id: null });
      expect(clearConnectionErrors(m).lastError).toBeNull();
    }
    const kept = applyControl(emptyModel(), { type: "Error", request: "CreateSession", message: "nope", session_id: "S1" });
    expect(clearConnectionErrors(kept)).toBe(kept);
    const empty = emptyModel();
    expect(clearConnectionErrors(empty)).toBe(empty);
  });
});

describe("ProjectUpdated", () => {
  it("replaces the project's fields, so a renamed project shows its new name", () => {
    const added = applyControl(emptyModel(), { type: "ProjectAdded", project: { id: "P1", path: "/srv/app", name: "app" } });
    const renamed = applyControl(added, { type: "ProjectUpdated", project: { id: "P1", path: "/srv/app", name: "Website" } });
    expect(renamed.projects.P1).toEqual({ id: "P1", path: "/srv/app", name: "Website" });
  });
});
