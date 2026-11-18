// Pure helpers for TerminalPane.svelte.
import { OS_NAME } from "../platform";
import type { SessionInfo } from "../protocol/SessionInfo";

/**
 * True when keyboard focus is in a text field outside the terminals: a
 * rename field in the sidebar or tab bar, or a dialog's input. A terminal
 * pane that mounts or becomes the focused pane then leaves focus where it
 * is. Without this, a pane that remounts while the user types a new name
 * (its session started, exited or was restored, which changes paneKey)
 * took focus, and the rename field's blur committed or dropped the
 * half-typed name. xterm.js's own textarea (inside `.xterm`) does not
 * count, so focus still moves from one terminal to another.
 */
export function typingOutsideTerminals(active: { tagName: string; closest(selector: string): unknown } | null): boolean {
  if (!active || (active.tagName !== "INPUT" && active.tagName !== "TEXTAREA")) return false;
  return active.closest(".xterm") === null;
}

/**
 * The `{#key}` value App.svelte mounts the active pane under. It includes
 * `attachable` so that a session losing its screen (jojod was replaced and
 * the new daemon lists it as not attachable) remounts the pane: the new
 * instance shows the no-screen message instead of the previous daemon's
 * last screen, and does not send input for a session the daemon cannot
 * attach.
 */
export function paneKey(session: Pick<SessionInfo, "id" | "attachable">): string {
  return `${session.id}:${session.attachable}`;
}

/**
 * True when jojod could not start the session's program. On a failed spawn
 * the daemon stores and broadcasts the session as exited with exit code -1
 * and never updates `state_changed_at`, which therefore still equals
 * `created_at`. A program that ran and exited with -1 has `state_changed_at`
 * set to its exit time by the daemon, and a session that was still running
 * when its jojod stopped is reloaded as exited with a null exit code, so
 * neither matches. A failed spawn from a previous jojod keeps these values
 * in state.json and matches too, which is correct: it also never started.
 */
export function failedToStart(session: Pick<SessionInfo, "state" | "exit_code" | "created_at" | "state_changed_at">): boolean {
  return session.state === "exited" && session.exit_code === -1 && session.state_changed_at === session.created_at;
}

/** The dimmed text a pane shows for a session that is not attachable. */
export function noScreenMessage(session: SessionInfo): string {
  if (failedToStart(session)) {
    return `This session has no screen: ${session.command} failed to start. Logs are in %LOCALAPPDATA%\\Jojo\\logs\\jojod.<date>.log. Close it and start a new one.`;
  }
  if (session.state === "exited" && session.exit_code === null) {
    return `This session ended when jojod stopped (for example, when ${OS_NAME} restarted). Right-click it in the sidebar ▸ Resume or Restart, or turn on Settings ▸ Restore sessions.`;
  }
  return "This session has no screen: it ran in a previous jojod. Close it and start a new one.";
}
