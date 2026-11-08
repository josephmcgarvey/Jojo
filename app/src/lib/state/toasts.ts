// When a session state change raises a Windows toast (spec 7.4).
import type { AgentState } from "../protocol/AgentState";
import type { SessionInfo } from "../protocol/SessionInfo";

/** Rule 4: at most one toast per session per 10 seconds. */
export const TOAST_INTERVAL_MS = 10_000;

/** States that announce themselves (rule 1). */
const ANNOUNCED: ReadonlySet<AgentState> = new Set<AgentState>(["needs_you", "done", "error"]);

export interface ToastContext {
  /** The session whose pane is focused in the active tab, if any. */
  focusedSessionId: string | null;
  /** Whether the Jojo window is the foreground window. */
  windowFocused: boolean;
  /** The `toasts` setting (rule 6). */
  enabled: boolean;
  /** Unix ms of this session's last toast, if any. */
  lastToastAt: number | undefined;
  now: number;
  projectName: string | undefined;
}

export interface ToastContent {
  title: string;
  body: string;
}

/**
 * The toast for a session whose state changed from `previous` to
 * `session.state`, or null when none is due: the session did not newly enter
 * needs_you, done or error; it is the focused pane of the foreground window;
 * toasts are off; or it had a toast less than 10 seconds ago.
 */
export function toastFor(previous: AgentState | undefined, session: SessionInfo, ctx: ToastContext): ToastContent | null {
  if (!ctx.enabled || previous === session.state || !ANNOUNCED.has(session.state)) return null;
  if (ctx.windowFocused && ctx.focusedSessionId === session.id) return null;
  if (ctx.lastToastAt !== undefined && ctx.now - ctx.lastToastAt < TOAST_INTERVAL_MS) return null;
  const title = ctx.projectName ? `${session.label} · ${ctx.projectName}` : session.label;
  const body =
    session.state === "done" ? "Finished" : (session.state_detail ?? (session.state === "needs_you" ? "Needs your input" : "Error"));
  return { title, body };
}

/** Badge tooltip: the state in words, plus its detail when there is one. */
export function stateTooltip(session: Pick<SessionInfo, "state" | "state_detail" | "exit_code">): string {
  const words: Record<AgentState, string> = {
    starting: "Starting",
    idle: "Idle",
    working: "Working",
    needs_you: "Needs you",
    done: "Done",
    error: "Error",
    running: "Running",
    exited: session.exit_code === null ? "Exited" : `Exited with code ${session.exit_code}`,
  };
  return session.state_detail ? `${words[session.state]}: ${session.state_detail}` : words[session.state];
}
