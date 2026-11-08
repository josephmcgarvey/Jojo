import { describe, expect, it } from "vitest";
import type { SessionInfo } from "../protocol/SessionInfo";
import { stateTooltip, TOAST_INTERVAL_MS, toastFor, type ToastContext } from "./toasts";

function session(over: Partial<SessionInfo> = {}): SessionInfo {
  return {
    id: "S1",
    project_id: "P1",
    kind: "claude",
    command: "claude.exe",
    args: [],
    label: "Fix the login bug",
    created_at: 0,
    state: "done",
    state_changed_at: 0,
    exit_code: null,
    claude_session_id: null,
    state_detail: null,
    renamed: false,
    attachable: true,
    ...over,
  };
}

const ctx = (over: Partial<ToastContext> = {}): ToastContext => ({
  focusedSessionId: "OTHER",
  windowFocused: true,
  enabled: true,
  lastToastAt: undefined,
  now: 100_000,
  projectName: "terminal-app",
  ...over,
});

describe("toastFor", () => {
  it("announces entering done, needs_you and error with the spec's title and bodies", () => {
    expect(toastFor("working", session(), ctx())).toEqual({ title: "Fix the login bug · terminal-app", body: "Finished" });
    expect(toastFor("working", session({ state: "needs_you", state_detail: "Needs permission: Bash(git push)" }), ctx())?.body).toBe(
      "Needs permission: Bash(git push)",
    );
    expect(toastFor("working", session({ state: "error", state_detail: "Error: rate_limit" }), ctx())?.body).toBe("Error: rate_limit");
  });

  it("stays quiet for other states and for updates that keep the state", () => {
    expect(toastFor("idle", session({ state: "working" }), ctx())).toBeNull();
    expect(toastFor("done", session({ state: "done" }), ctx())).toBeNull();
  });

  it("stays quiet for the focused pane of the foreground window only", () => {
    expect(toastFor("working", session(), ctx({ focusedSessionId: "S1" }))).toBeNull();
    expect(toastFor("working", session(), ctx({ focusedSessionId: "S1", windowFocused: false }))).not.toBeNull();
  });

  it("limits each session to one toast per ten seconds", () => {
    expect(toastFor("working", session(), ctx({ lastToastAt: 100_000 - TOAST_INTERVAL_MS + 1 }))).toBeNull();
    expect(toastFor("working", session(), ctx({ lastToastAt: 100_000 - TOAST_INTERVAL_MS }))).not.toBeNull();
  });

  it("respects the toasts setting", () => {
    expect(toastFor("working", session(), ctx({ enabled: false }))).toBeNull();
  });
});

describe("stateTooltip", () => {
  it("names the state and adds its detail", () => {
    expect(stateTooltip({ state: "needs_you", state_detail: "Needs permission: Bash(ls)", exit_code: null })).toBe("Needs you: Needs permission: Bash(ls)");
    expect(stateTooltip({ state: "exited", state_detail: null, exit_code: 3 })).toBe("Exited with code 3");
    expect(stateTooltip({ state: "working", state_detail: null, exit_code: null })).toBe("Working");
  });
});
