// Routes Output and Snapshot frames to the mounted pane of their session.
import type { Frame } from "../wire/frame";

export interface PaneHandle {
  deliver(kind: "output" | "snapshot", bytes: Uint8Array): void;
  reattach(): void;
}

export function createRouter() {
  const panes = new Map<string, PaneHandle>();
  return {
    register(sessionId: string, pane: PaneHandle): () => void {
      panes.set(sessionId, pane);
      return () => {
        if (panes.get(sessionId) === pane) panes.delete(sessionId);
      };
    },
    deliver(frame: Frame): void {
      if (frame.kind === "output" || frame.kind === "snapshot") panes.get(frame.sessionId)?.deliver(frame.kind, frame.bytes);
    },
    /** jojod detached a client that fell behind; attach again for a fresh snapshot. */
    fellBehind(sessionId: string | null): void {
      if (sessionId) panes.get(sessionId)?.reattach();
    },
    /** After a reconnect, every mounted pane attaches again. */
    reattachAll(): void {
      for (const pane of panes.values()) pane.reattach();
    },
  };
}

export const router = createRouter();
