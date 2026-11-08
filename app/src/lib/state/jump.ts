// Ctrl+Shift+J (spec 9.6): jump to the next session that needs attention.
import type { SessionInfo } from "../protocol/SessionInfo";

/**
 * The session Ctrl+Shift+J focuses: the next session in `needs_you` after
 * the focused one (in `ordered`, wrapping around), or else the next in
 * `done`. Returns null when no other session needs attention. The focused
 * session itself is chosen only when it is the one session in that state.
 */
export function nextAttentionSession(ordered: Pick<SessionInfo, "id" | "state">[], focusedId: string | null): string | null {
  const start = focusedId === null ? -1 : ordered.findIndex((s) => s.id === focusedId);
  const rotated = [...ordered.slice(start + 1), ...ordered.slice(0, start + 1)];
  for (const state of ["needs_you", "done"] as const) {
    const next = rotated.find((s) => s.state === state);
    if (next) return next.id;
  }
  return null;
}
