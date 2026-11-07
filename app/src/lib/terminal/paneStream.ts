// The data path of one mounted pane: which frames reach term.write(), and
// when to send Attach. TerminalPane.svelte supplies the terminal write and
// the Attach request; everything else is here so it can be tested without
// xterm.js.
import { AttachGate } from "./attachGate";
import { WriteFlow } from "./flowControl";
import type { PaneHandle } from "./router";

export interface PaneStreamDeps {
  /** term.write(bytes, done): `done` runs once xterm.js has parsed `bytes`. */
  write(bytes: Uint8Array, done: () => void): void;
  /** Sends Attach with the pane's current size; rejects when the bridge refuses (disconnected). */
  sendAttach(): Promise<void>;
}

export interface PaneStream extends PaneHandle {
  /** Sends the first Attach. */
  start(): void;
}

export function createPaneStream(deps: PaneStreamDeps): PaneStream {
  const gate = new AttachGate();
  const flow = new WriteFlow();
  const attach = (): void => {
    if (!gate.begin()) return;
    deps.sendAttach().catch(() => gate.abandon());
  };
  return {
    start: attach,
    deliver(kind, bytes) {
      const accepted = gate.accept(kind, bytes);
      if (!accepted) return;
      // While behind, Output is dropped; the Attach sent on catching up
      // brings a Snapshot that covers it. Snapshots are always written.
      if (kind === "output" && flow.isBehind) return;
      flow.wrote(accepted.length);
      deps.write(accepted, () => {
        if (flow.parsed(accepted.length)) attach();
      });
    },
    reattach(afterReconnect) {
      // An Attach outstanding on a connection that has since closed will
      // never be answered.
      if (afterReconnect) gate.abandon();
      attach();
    },
    attachFailed() {
      gate.abandon();
    },
  };
}
