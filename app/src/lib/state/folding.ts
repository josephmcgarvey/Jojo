// Folding sidebar groups (a location's projects, a project's sessions):
// layout.json's `collapsed` list holds the ids of the folded locations and
// projects. Kept free of app state so the rules can be tested.
import type { AgentState } from "../protocol/AgentState";
import type { SessionInfo } from "../protocol/SessionInfo";

/** `collapsed` with every id in `ids` folded (`fold`) or unfolded, without duplicates; the order of the other ids is kept. */
export function setFolded(collapsed: readonly string[], ids: readonly string[], fold: boolean): string[] {
  const rest = collapsed.filter((id) => !ids.includes(id));
  return fold ? [...rest, ...ids.filter((id, i) => ids.indexOf(id) === i)] : rest;
}

/** States shown as a dot on a folded row: the ones with a state color (attention.ts stateToken). */
const SUMMARY_STATES: ReadonlySet<AgentState> = new Set<AgentState>(["needs_you", "working", "done", "error"]);

/**
 * The dots a folded row shows in place of its hidden sessions: one per
 * session whose state has a color, in the order the sessions are listed.
 * Idle, starting, running and exited sessions show none.
 */
export function foldedStates(sessions: readonly Pick<SessionInfo, "state">[]): AgentState[] {
  return sessions.map((s) => s.state).filter((state) => SUMMARY_STATES.has(state));
}
