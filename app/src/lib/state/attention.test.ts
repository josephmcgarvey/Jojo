import { describe, expect, it } from "vitest";
import type { AgentState } from "../protocol/AgentState";
import type { SessionInfo } from "../protocol/SessionInfo";
import { needsYouQueue, stateSummary, stateToken, stripGroups } from "./attention";

function session(id: string, state: AgentState, changedAt = 0): SessionInfo {
  return {
    id,
    project_id: "P",
    kind: "claude",
    command: "claude",
    args: [],
    label: id,
    created_at: 0,
    state,
    state_changed_at: changedAt,
    exit_code: null,
    claude_session_id: null,
    state_detail: null,
    renamed: false,
    prompt_label: null,
    context_tokens: null,
    context_window: null,
    attachable: true,
  };
}

describe("attention", () => {
  it("colors the states that have a badge color and leaves the rest plain", () => {
    expect(stateToken("needs_you")).toBe("--jojo-needs-you");
    expect(stateToken("working")).toBe("--jojo-working");
    expect(stateToken("done")).toBe("--jojo-done");
    expect(stateToken("error")).toBe("--jojo-error");
    expect(stateToken("idle")).toBeNull();
    expect(stateToken("running")).toBeNull();
  });

  it("queues the sessions that need you, longest wait first", () => {
    const queue = needsYouQueue([session("a", "needs_you", 300), session("b", "working", 100), session("c", "needs_you", 100)]);
    expect(queue.map((s) => s.id)).toEqual(["c", "a"]);
  });

  it("groups the strip by machine in sidebar order and leaves out exited sessions", () => {
    const groups = stripGroups([
      { session: session("a", "working"), machineId: "local" },
      { session: session("b", "exited"), machineId: "local" },
      { session: session("c", "idle"), machineId: "local" },
      { session: session("d", "exited"), machineId: "srv" },
      { session: session("e", "done"), machineId: "srv2" },
    ]);
    expect(groups.map((g) => [g.machineId, g.sessions.map((s) => s.id)])).toEqual([
      ["local", ["a", "c"]],
      ["srv2", ["e"]],
    ]);
  });

  it("summarizes the states, most urgent first", () => {
    const all = [session("a", "idle"), session("b", "needs_you"), session("c", "working"), session("d", "needs_you"), session("e", "done")];
    expect(stateSummary(all)).toBe("2 need you · 1 working · 1 done · 1 idle");
    expect(stateSummary([session("a", "needs_you")])).toBe("1 needs you");
    expect(stateSummary([])).toBe("");
  });
});
