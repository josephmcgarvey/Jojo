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
