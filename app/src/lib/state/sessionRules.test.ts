import { describe, expect, it } from "vitest";
import type { SessionInfo } from "../protocol/SessionInfo";
import {
  busyStateText,
  canRestart,
  closeTabMessage,
  closeTabNeedsConfirmation,
  closeTabSummary,
  closesOnExit,
  isBusy,
  restartShell,
} from "./sessionRules";

function session(over: Partial<SessionInfo>): SessionInfo {
  return {
    id: "S1",
    project_id: "P1",
    kind: "claude",
    command: "claude.exe",
    args: [],
    label: "claude",
    created_at: 1,
    state: "idle",
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

describe("sessionRules", () => {
  it("treats working and needs_you as busy and nothing else", () => {
    expect(isBusy(session({ state: "working" }))).toBe(true);
    expect(isBusy(session({ state: "needs_you" }))).toBe(true);
    for (const state of ["starting", "idle", "done", "error", "running", "exited"] as const) {
      expect(isBusy(session({ state })), state).toBe(false);
    }
    expect(busyStateText("needs_you")).toBe("waiting for you");
    expect(busyStateText("working")).toBe("working");
  });

  it("summarizes a tab and asks only for several sessions or a busy one", () => {
    const one = closeTabSummary([session({ label: "a" })]);
    expect(one).toEqual({ count: 1, busy: 0, labels: ["a"] });
    expect(closeTabNeedsConfirmation(one)).toBe(false);
    const two = closeTabSummary([session({ label: "a", state: "needs_you" }), session({ label: "b" })]);
    expect(two).toEqual({ count: 2, busy: 1, labels: ["a (waiting for you)", "b"] });
    expect(closeTabNeedsConfirmation(two)).toBe(true);
    expect(closeTabMessage(two)).toBe("Closing this tab ends 2 sessions. 1 of them is working or waiting for you:");
    expect(closeTabMessage(closeTabSummary([session({}), session({})]))).toBe("Closing this tab ends 2 sessions:");
    expect(closeTabNeedsConfirmation(closeTabSummary([session({ state: "working" })]))).toBe(true);
  });

  it("restarts only exited sessions, with the same shell program for shells", () => {
    expect(canRestart(session({ state: "exited" }))).toBe(true);
    expect(canRestart(session({ state: "running" }))).toBe(false);
    expect(restartShell(session({ kind: "shell", command: "pwsh" }))).toBe("pwsh");
    expect(restartShell(session({ kind: "claude", command: "cmd.exe" }))).toBeNull();
  });

  it("closes the pane only when the session exited with code 0 (Amendment A)", () => {
    expect(closesOnExit(0)).toBe(true);
    expect(closesOnExit(1)).toBe(false);
    expect(closesOnExit(-1)).toBe(false);
    expect(closesOnExit(null)).toBe(false);
  });
});
