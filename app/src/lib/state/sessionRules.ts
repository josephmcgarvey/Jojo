// Rules about sessions that the menus and close actions share.
import type { AgentState } from "../protocol/AgentState";
import type { SessionInfo } from "../protocol/SessionInfo";

/**
 * States in which closing a session asks for confirmation first (spec 9.3
 * rule 6, spec 9.4 Close tab). Only Claude Code hooks put a session in
 * these states, and Plan 4 adds the hooks; until then no session is in
 * either state and closing never asks. The rule reads `SessionInfo.state`,
 * so Plan 4 needs no change here.
 */
const BUSY_STATES: ReadonlySet<AgentState> = new Set<AgentState>(["working", "needs_you"]);

export function isBusy(session: Pick<SessionInfo, "state">): boolean {
  return BUSY_STATES.has(session.state);
}

export function busyStateText(state: AgentState): string {
  return state === "needs_you" ? "waiting for you" : state;
}

export interface CloseTabSummary {
  count: number;
  busy: number;
  /** One line per session; busy sessions name their state. */
  labels: string[];
}

export function closeTabSummary(sessions: SessionInfo[]): CloseTabSummary {
  return {
    count: sessions.length,
    busy: sessions.filter(isBusy).length,
    labels: sessions.map((s) => (isBusy(s) ? `${s.label} (${busyStateText(s.state)})` : s.label)),
  };
}

export function closeTabMessage(summary: CloseTabSummary): string {
  const sessions = summary.count === 1 ? "1 session" : `${summary.count} sessions`;
  if (summary.busy === 0) return `Closing this tab ends ${sessions}:`;
  const verb = summary.busy === 1 ? "is" : "are";
  return `Closing this tab ends ${sessions}. ${summary.busy} of them ${verb} working or waiting for you:`;
}

/**
 * Close tab asks first when it would end more than one session or any busy
 * session. Closing a tab that holds one idle session needs no confirmation,
 * for the same reason closing one idle pane needs none (spec 9.3 rule 6).
 */
export function closeTabNeedsConfirmation(summary: CloseTabSummary): boolean {
  return summary.count > 1 || summary.busy > 0;
}

/** Spec 5.4: Restart is offered for sessions in `exited`. */
/**
 * Spec 5.4: Resume is offered for an exited session with a recorded
 * claude_session_id. That includes a shell session in which `claude` was
 * started by hand; Resume then starts claude directly in its pane.
 */
export function canResume(session: Pick<SessionInfo, "state" | "claude_session_id">): boolean {
  return session.state === "exited" && !!session.claude_session_id;
}

export function canRestart(session: Pick<SessionInfo, "state">): boolean {
  return session.state === "exited";
}

/**
 * The `shell` field of a restart's CreateSession. A shell session passes its
 * own program, which jojod uses verbatim (`command_for` in
 * crates/jojod/src/command.rs). A claude session passes null, because jojod
 * resolves `claude.exe` or `cmd.exe /d /c claude` itself.
 */
export function restartShell(session: Pick<SessionInfo, "kind" | "command">): string | null {
  return session.kind === "shell" ? session.command : null;
}

/** Amendment A (user request): a session that exits with code 0 closes its pane; any other exit code, or none, keeps the pane so its final output stays readable (spec 5.3). */
export function closesOnExit(exitCode: number | null): boolean {
  return exitCode === 0;
}
