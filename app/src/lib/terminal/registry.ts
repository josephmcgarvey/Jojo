// The mounted terminals by session id, so menus and shortcuts can act on a
// pane's terminal (copy, paste, selection, mouse mode, focus). A pane that
// is not mounted (hidden tab, zoomed away) has no entry.
import type { MouseTrackingMode } from "../input/mouse";

export interface TerminalControls {
  focus(): void;
  hasSelection(): boolean;
  getSelection(): string;
  paste(text: string): void;
  mouseTrackingMode(): MouseTrackingMode;
  /** Applies new terminal font settings and refits the terminal to its pane. */
  applyFont(family: string, size: number, lineHeight: number): void;
  /** Spec 10, Debug ▸ Rehydrate pane: requests a new snapshot, which repaints the terminal. */
  rehydrate(): void;
}

const terminals = new Map<string, TerminalControls>();

export function registerTerminal(sessionId: string, controls: TerminalControls): () => void {
  terminals.set(sessionId, controls);
  return () => {
    if (terminals.get(sessionId) === controls) terminals.delete(sessionId);
  };
}

export function terminalControls(sessionId: string | null): TerminalControls | undefined {
  return sessionId ? terminals.get(sessionId) : undefined;
}

/** Every mounted terminal (settings changes apply to all of them). */
export function allTerminals(): TerminalControls[] {
  return [...terminals.values()];
}
