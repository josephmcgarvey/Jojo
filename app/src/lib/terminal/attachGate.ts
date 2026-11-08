// Decides which Snapshot and Output frames a pane writes into its terminal
// (Plan 2a carry-over 2.1 item 2; Plan 2b carry-over 3.2 item 1).
//
// jojod answers every Attach with exactly one Snapshot, and live Output for
// the session follows that Snapshot. Output that arrives while an Attach is
// outstanding belongs to the previous subscription (queued before a Detach,
// or sent before jojod dropped this client for falling behind) and is
// dropped; the Snapshot replaces it, because a snapshot starts with a full
// terminal reset.
//
// jojod's per-client queue accounting (crates/jojod/src/client_queue.rs
// counts a Snapshot as 256 bytes) assumes the app keeps at most one Attach
// outstanding per session. The gate counts Attaches sent and Snapshots
// received, and `begin()` refuses a second Attach while they differ; the
// outstanding Attach's Snapshot brings the pane up to date anyway.
// Snapshots carry no request id, so a Snapshot that arrives when none is
// outstanding (left from a pane instance that was unmounted before its
// Snapshot arrived) is written but not counted.
export class AttachGate {
  private sent = 0;
  private answered = 0;

  get outstanding(): boolean {
    return this.sent > this.answered;
  }

  /** True when the caller should send an Attach now; false while one is outstanding. */
  begin(): boolean {
    if (this.outstanding) return false;
    this.sent += 1;
    return true;
  }

  /**
   * The outstanding Attach will never be answered: the bridge refused to
   * send it, jojod replied with an Attach error, or the connection it was
   * sent on has closed.
   */
  abandon(): void {
    this.answered = this.sent;
  }

  accept(kind: "output" | "snapshot", bytes: Uint8Array): Uint8Array | null {
    if (kind === "snapshot") {
      this.answered = Math.min(this.answered + 1, this.sent);
      return bytes;
    }
    return this.outstanding ? null : bytes;
  }
}
