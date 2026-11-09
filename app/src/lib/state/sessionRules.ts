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

/**
 * Spec 5.4: Resume is offered for an exited session with a recorded
 * claude_session_id. That includes a shell session in which `claude` was
 * started by hand; Resume then starts claude directly in its pane.
 */
export function canResume(session: Pick<SessionInfo, "state" | "claude_session_id">): boolean {
  return session.state === "exited" && !!session.claude_session_id;
}

/** Spec 5.4: Restart is offered for sessions in `exited`. */
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

/** A shell that exits sooner than this after starting keeps its pane, so a startup error stays readable. */
export const SHELL_STARTUP_MS = 3_000;

/**
 * Amendment A (user request): whether a session's exit closes its pane.
 *
 * 1. No exit code (the daemon stopped, or the exit was not observed): the
 *    pane stays.
 * 2. Exit code 0: the pane closes.
 * 3. A shell session that ran at least SHELL_STARTUP_MS: the pane closes
 *    whatever the code. A shell ends only when told to (`exit`, Ctrl+D),
 *    and zsh's and bash's `exit` returns the status of the last command
 *    run, so `exit` after a failed command (127 for a command not found,
 *    130 after Ctrl+C) reports a non-zero code. PowerShell's `exit`
 *    returns 0, which is why rule 2 alone worked on Windows.
 * 4. Otherwise (a Claude session that failed, or a shell that exited
 *    during startup, for example from an error in ~/.zshrc): the pane
 *    stays, so the final output stays readable (spec 5.3).
 */
export function closesOnExit(session: Pick<SessionInfo, "kind" | "created_at"> | undefined, exitCode: number | null, now: number): boolean {
  if (exitCode === null) return false;
  if (exitCode === 0) return true;
  return session?.kind === "shell" && now - session.created_at >= SHELL_STARTUP_MS;
}

/** How a session that ended with its daemon is brought back (see `restorePlan`). */
export interface RestorePlan {
  kind: "claude" | "shell";
  /** CreateSession.shell: the old session's own program for a shell. */
  shell: string | null;
  /** CreateSession.resume: the conversation a claude session continues. */
  resume: string | null;
  /** Typed into the new shell once it starts: `claude --resume <id>`. */
  typeAfterStart: string | null;
}

/**
 * Restoring sessions after Windows or jojod restarts (user request, like
 * Herdr). A session qualifies when it is exited with no exit code, which
 * is how the daemon marks sessions that were still running when it stopped
 * (crates/jojod/src/store.rs); a session that crashed or that the user
 * exited has a code and is left alone. A claude session continues its
 * conversation with `claude --resume`. A shell restarts with its own
 * program in its folder; when Claude Code was running in it (the daemon
 * keeps a shell's claude_session_id only while Claude Code runs there),
 * `claude --resume <id>` is typed into the new shell, so the shell remains
 * when Claude exits. Conversation ids are accepted only as letters, digits
 * and hyphens, because the id is typed into a shell.
 */
export function restorePlan(session: Pick<SessionInfo, "state" | "exit_code" | "kind" | "command" | "claude_session_id">): RestorePlan | null {
  if (session.state !== "exited" || session.exit_code !== null) return null;
  const id = session.claude_session_id && /^[A-Za-z0-9-]{1,128}$/.test(session.claude_session_id) ? session.claude_session_id : null;
  if (session.kind === "claude") return { kind: "claude", shell: null, resume: id, typeAfterStart: null };
  return { kind: "shell", shell: session.command, resume: null, typeAfterStart: id ? `claude --resume ${id}\r` : null };
}

