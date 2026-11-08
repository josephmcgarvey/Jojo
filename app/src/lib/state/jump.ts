// Ctrl+Shift+J (spec 9.6): jump to the next session that needs attention.
import type { SessionInfo } from "../protocol/SessionInfo";

/**
 * The session Ctrl+Shift+J focuses: the next session in `needs_you` after
 * the focused one (in `ordered`, wrapping around), or else the next in
 * `done`. The focused session is skipped, so from the only `needs_you`
 * session the jump reaches the `done` ones; it is returned only when no
 * other session needs attention. Null when none does.
 */
export function nextAttentionSession(ordered: Pick<SessionInfo, "id" | "state">[], focusedId: string | null): string | null {
  const start = focusedId === null ? -1 : ordered.findIndex((s) => s.id === focusedId);
  const others = [...ordered.slice(start + 1), ...ordered.slice(0, Math.max(start, 0))].filter((s) => s.id !== focusedId);
  for (const state of ["needs_you", "done"] as const) {
    const next = others.find((s) => s.state === state);
    if (next) return next.id;
  }
  // Nothing else needs attention: stay on the focused session if it does.
  const focused = ordered.find((s) => s.id === focusedId);
  return focused && (focused.state === "needs_you" || focused.state === "done") ? focused.id : null;
}
