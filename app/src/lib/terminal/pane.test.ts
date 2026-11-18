import { expect, it } from "vitest";
import type { SessionInfo } from "../protocol/SessionInfo";
import { noScreenMessage, paneKey, typingOutsideTerminals } from "./pane";

/** A stand-in for document.activeElement: its tag and whether it is inside a terminal (`.xterm`). */
function active(tagName: string, inTerminal = false) {
  return { tagName, closest: (selector: string) => (selector === ".xterm" && inTerminal ? {} : null) };
}

it("keeps a terminal from taking focus from a rename field or a dialog's text field", () => {
  expect(typingOutsideTerminals(active("INPUT"))).toBe(true);
  expect(typingOutsideTerminals(active("TEXTAREA"))).toBe(true);
  expect(typingOutsideTerminals(active("SELECT"))).toBe(false);
  // xterm.js takes keys through its own textarea; another terminal may take focus from it.
  expect(typingOutsideTerminals(active("TEXTAREA", true))).toBe(false);
  expect(typingOutsideTerminals(active("BUTTON"))).toBe(false);
  expect(typingOutsideTerminals(active("BODY"))).toBe(false);
  expect(typingOutsideTerminals(null)).toBe(false);
});

it("changes the pane key when a session stops being attachable", () => {
  const before = paneKey({ id: "S1", attachable: true });
  const after = paneKey({ id: "S1", attachable: false });
  expect(before).not.toBe(after);
  expect(paneKey({ id: "S1", attachable: true })).toBe(before);
  expect(paneKey({ id: "S2", attachable: true })).not.toBe(before);
});

function session(over: Partial<SessionInfo>): SessionInfo {
  return {
    id: "S1",
    project_id: "P1",
    kind: "shell",
    command: "pwsh.exe",
    args: [],
    label: "pwsh",
    created_at: 1000,
    state: "exited",
    state_changed_at: 1000,
    exit_code: -1,
    claude_session_id: null,
    state_detail: null,
    renamed: false,
    prompt_label: null, context_tokens: null, context_window: null,
    attachable: false,
    ...over,
  };
}

it("says a failed spawn failed to start and points at the log", () => {
  const message = noScreenMessage(session({}));
  expect(message).toContain("pwsh.exe failed to start");
  expect(message).toContain(String.raw`%LOCALAPPDATA%\Jojo\logs\jojod.<date>.log`);
});

it("explains a session that ended with its jojod, and says any other ran in a previous jojod", () => {
  const previous = "This session has no screen: it ran in a previous jojod. Close it and start a new one.";
  // Running when its jojod stopped: reloaded as exited with no exit code.
  expect(noScreenMessage(session({ exit_code: null, state_changed_at: 2000 }))).toContain("ended when jojod stopped");
  // Ran, then exited with -1: state_changed_at is its exit time.
  expect(noScreenMessage(session({ state_changed_at: 2000 }))).toBe(previous);
  expect(noScreenMessage(session({ exit_code: 0, state_changed_at: 2000 }))).toBe(previous);
});
