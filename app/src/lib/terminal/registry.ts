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
