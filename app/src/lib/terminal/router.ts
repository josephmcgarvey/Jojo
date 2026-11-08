// Routes Output and Snapshot frames to the mounted pane of their session.
import type { Frame } from "../wire/frame";

export interface PaneHandle {
  deliver(kind: "output" | "snapshot", bytes: Uint8Array): void;
  /** Attach again for a fresh Snapshot; `afterReconnect` discards an Attach outstanding on the old connection. */
  reattach(afterReconnect: boolean): void;
  /** jojod answered this pane's Attach with an error, so no Snapshot is coming. */
  attachFailed(): void;
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
      if (sessionId) panes.get(sessionId)?.reattach(false);
    },
    /** After a reconnect, every mounted pane attaches again. */
    /** After a reconnect: attaches again every pane, or only those `only` accepts (one machine's sessions). */
    reattachAll(only?: (sessionId: string) => boolean): void {
      for (const [sessionId, pane] of panes) {
        if (!only || only(sessionId)) pane.reattach(true);
      }
    },
    attachFailed(sessionId: string): void {
      panes.get(sessionId)?.attachFailed();
    },
  };
}

export const router = createRouter();
