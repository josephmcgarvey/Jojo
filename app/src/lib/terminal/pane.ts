// Pure helpers for TerminalPane.svelte.
import type { SessionInfo } from "../protocol/SessionInfo";

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
  return "This session has no screen: it ran in a previous jojod. Close it and start a new one.";
}
