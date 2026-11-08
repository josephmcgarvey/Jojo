// Counts bytes handed to term.write() that xterm.js has not yet parsed
// (Plan 2b carry-over 3.2 item 2). xterm.js throws "write data discarded,
// use flow control to avoid losing data" once more than 50,000,000 bytes
// wait in its write queue. Above HIGH_WATERMARK the pane drops Output; at
// LOW_WATERMARK after that it attaches again, and the Snapshot replaces the
// dropped Output.

/**
 * Above the largest Snapshot jojod sends (MAX_SNAPSHOT_BYTES, 16 MiB minus
 * 17 bytes), so a Snapshot alone never counts as falling behind, and low
 * enough that a full Snapshot written on top of it stays below xterm.js's
 * 50,000,000-byte limit.
 */
export const HIGH_WATERMARK = 24 * 1024 * 1024;

/** Queue size at which a pane that fell behind attaches again. */
export const LOW_WATERMARK = 2 * 1024 * 1024;

export class WriteFlow {
  private pending = 0;
  private behind = false;

  get isBehind(): boolean {
    return this.behind;
  }

  get pendingBytes(): number {
    return this.pending;
  }

  /** Records bytes passed to term.write(); true when this write crossed HIGH_WATERMARK. */
  wrote(n: number): boolean {
    this.pending += n;
    if (!this.behind && this.pending > HIGH_WATERMARK) {
      this.behind = true;
      return true;
    }
    return false;
  }

  /** Records a term.write() callback; true when a pane that fell behind has drained to LOW_WATERMARK. */
  parsed(n: number): boolean {
    this.pending = Math.max(0, this.pending - n);
    if (this.behind && this.pending <= LOW_WATERMARK) {
      this.behind = false;
      return true;
    }
    return false;
  }
}
