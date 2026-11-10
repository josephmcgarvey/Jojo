// Shared rules for the parts of the window that show which sessions want
// attention (the "agents first" layout): the state color a pane glows in,
// the Needs you queue at the top of the sidebar, the state strip along the
// bottom, and the short state words on cards and pane headers.
import type { AgentState } from "../protocol/AgentState";
import type { SessionInfo } from "../protocol/SessionInfo";

/** Short words for a state, as cards, pane headers and the strip show them. */
export const STATE_WORDS: Record<AgentState, string> = {
  starting: "Starting",
  idle: "Idle",
  working: "Working",
  needs_you: "Needs you",
  done: "Done",
  error: "Error",
  running: "Running",
  exited: "Exited",
};

/**
 * The theme token a state is drawn in, or null for states without their own
 * color (idle, running, starting, exited), which keep the plain pane border.
 * These are the badge colors, so a pane, a card and its badge always agree.
 */
export function stateToken(state: AgentState): string | null {
  switch (state) {
    case "needs_you":
      return "--jojo-needs-you";
    case "working":
      return "--jojo-working";
    case "done":
      return "--jojo-done";
    case "error":
      return "--jojo-error";
    default:
      return null;
  }
}

/**
 * The Needs you queue: every session waiting on the user, oldest wait
 * first, so the one that has waited longest is at the top.
 */
export function needsYouQueue(sessions: Iterable<SessionInfo>): SessionInfo[] {
  return [...sessions].filter((s) => s.state === "needs_you").sort((a, b) => a.state_changed_at - b.state_changed_at);
}

export interface StripGroup {
  machineId: string;
  sessions: SessionInfo[];
}

/**
 * The state strip: the sessions that have not exited, in sidebar order
 * (`ordered`, from views.orderedSessions), grouped by machine so the strip
 * can leave a gap between locations. Machines without live sessions are
 * left out.
 */
export function stripGroups(ordered: { session: SessionInfo; machineId: string }[]): StripGroup[] {
  const groups: StripGroup[] = [];
  for (const { session, machineId } of ordered) {
    if (session.state === "exited") continue;
    const last = groups[groups.length - 1];
    if (last && last.machineId === machineId) last.sessions.push(session);
    else groups.push({ machineId, sessions: [session] });
  }
  return groups;
}

/** Order of the states in the strip's summary, most urgent first. */
const SUMMARY_ORDER: AgentState[] = ["needs_you", "error", "working", "done", "starting", "idle", "running"];

/** "2 need you · 1 working · 1 done": how many sessions are in each state, most urgent first; empty when there are none. */
export function stateSummary(sessions: SessionInfo[]): string {
  const counts = new Map<AgentState, number>();
  for (const s of sessions) counts.set(s.state, (counts.get(s.state) ?? 0) + 1);
  return SUMMARY_ORDER.filter((state) => counts.has(state))
    .map((state) => {
      const n = counts.get(state)!;
      const word = state === "needs_you" ? (n === 1 ? "needs you" : "need you") : STATE_WORDS[state].toLowerCase();
      return `${n} ${word}`;
    })
    .join(" · ");
}
