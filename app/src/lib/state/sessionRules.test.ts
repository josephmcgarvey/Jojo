import { describe, expect, it } from "vitest";
import type { SessionInfo } from "../protocol/SessionInfo";
import {
  restorePlan,
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

  it("closes the pane on exit code 0, and on any exit of a shell past startup (Amendment A)", () => {
    const shell = { kind: "shell" as const, created_at: 1_000 };
    const claude = { kind: "claude" as const, created_at: 1_000 };
    expect(closesOnExit(claude, 0, 60_000)).toBe(true);
    expect(closesOnExit(claude, 1, 60_000)).toBe(false);
    expect(closesOnExit(claude, null, 60_000)).toBe(false);
    // zsh's `exit` after a failed command reports that command's status.
    expect(closesOnExit(shell, 127, 60_000)).toBe(true);
    expect(closesOnExit(shell, 130, 60_000)).toBe(true);
    expect(closesOnExit(shell, null, 60_000)).toBe(false);
    // A shell that dies during startup keeps its error output on screen.
    expect(closesOnExit(shell, 1, 2_000)).toBe(false);
    expect(closesOnExit(undefined, 1, 60_000)).toBe(false);
  });
});

describe("restorePlan", () => {
  const ended = { state: "exited" as const, exit_code: null, kind: "shell" as const, command: "pwsh.exe", claude_session_id: null };

  it("restarts a terminal with its own shell, and resumes Claude in it when Claude was running there", () => {
    expect(restorePlan(ended)).toEqual({ kind: "shell", shell: "pwsh.exe", resume: null, typeAfterStart: null });
    expect(restorePlan({ ...ended, claude_session_id: "0f3c2a9e-1b2c" })?.typeAfterStart).toBe("claude --resume 0f3c2a9e-1b2c\r");
  });

  it("resumes a claude session's conversation", () => {
    expect(restorePlan({ ...ended, kind: "claude", command: "claude.exe", claude_session_id: "abc-1" })).toEqual({
      kind: "claude",
      shell: null,
      resume: "abc-1",
      typeAfterStart: null,
    });
  });

  it("leaves sessions that crashed, exited on their own or are still running alone", () => {
    expect(restorePlan({ ...ended, exit_code: 0 })).toBeNull();
    expect(restorePlan({ ...ended, exit_code: 1 })).toBeNull();
    expect(restorePlan({ ...ended, state: "running" })).toBeNull();
  });

  it("does not type an id that is not a plain conversation id", () => {
    expect(restorePlan({ ...ended, claude_session_id: "x; rm -rf ~" })?.typeAfterStart).toBeNull();
  });
});
