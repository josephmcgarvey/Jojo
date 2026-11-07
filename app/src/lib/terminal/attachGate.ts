// Output queued before a Detach can still arrive after a new Attach. Until
// the Snapshot answering the latest Attach arrives, Output is stale and is
// dropped (Plan 2a carry-over 2.1, item 2). A snapshot starts with a full
// reset, so it can be written into the same terminal.
export class AttachGate {
  private awaitingSnapshot = false;

  requestAttach(): void {
    this.awaitingSnapshot = true;
  }

  accept(kind: "output" | "snapshot", bytes: Uint8Array): Uint8Array | null {
    if (kind === "snapshot") {
      this.awaitingSnapshot = false;
      return bytes;
    }
    return this.awaitingSnapshot ? null : bytes;
  }
}
