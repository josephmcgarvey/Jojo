import { describe, expect, it } from "vitest";
import { HIGH_WATERMARK } from "./flowControl";
import { createPaneStream } from "./paneStream";

function harness() {
  const writes: { bytes: Uint8Array; done: () => void }[] = [];
  const attaches: { resolve: () => void; reject: (err: unknown) => void }[] = [];
  const stream = createPaneStream({
    write: (bytes, done) => writes.push({ bytes, done }),
    sendAttach: () => new Promise<void>((resolve, reject) => attaches.push({ resolve, reject })),
  });
  return { stream, writes, attaches };
}

const b = (...bytes: number[]) => new Uint8Array(bytes);
const tick = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("createPaneStream", () => {
  it("sends one Attach and writes nothing until its snapshot", () => {
    const { stream, writes, attaches } = harness();
    stream.start();
    expect(attaches).toHaveLength(1);
    stream.deliver("output", b(1));
    expect(writes).toHaveLength(0);
    stream.deliver("snapshot", b(2));
    stream.deliver("output", b(3));
    expect(writes.map((w) => [...w.bytes])).toEqual([[2], [3]]);
  });

  it("does not send a second Attach while one is outstanding", () => {
    const { stream, attaches } = harness();
    stream.start();
    stream.reattach(false);
    stream.reattach(false);
    expect(attaches).toHaveLength(1);
    stream.deliver("snapshot", b(1));
    stream.reattach(false);
    expect(attaches).toHaveLength(2);
  });

  it("sends a new Attach after a reconnect even if one was outstanding", () => {
    const { stream, attaches } = harness();
    stream.start();
    stream.reattach(true);
    expect(attaches).toHaveLength(2);
  });

  it("can retry an Attach the bridge refused", async () => {
    const { stream, attaches } = harness();
    stream.start();
    attaches[0].reject(new Error("not connected to jojod"));
    await tick();
    stream.reattach(false);
    expect(attaches).toHaveLength(2);
  });

  it("can retry after jojod refuses the Attach", () => {
    const { stream, attaches } = harness();
    stream.start();
    stream.attachFailed();
    stream.reattach(false);
    expect(attaches).toHaveLength(2);
  });

  it("drops output above the high watermark and resyncs once the terminal catches up", () => {
    const { stream, writes, attaches } = harness();
    stream.start();
    stream.deliver("snapshot", b(0));
    writes[0].done();
    stream.deliver("output", new Uint8Array(HIGH_WATERMARK));
    stream.deliver("output", b(1));
    expect(writes).toHaveLength(3);
    stream.deliver("output", b(2));
    expect(writes).toHaveLength(3);
    writes[1].done();
    expect(attaches).toHaveLength(2);
    stream.deliver("output", b(3));
    expect(writes).toHaveLength(3);
    stream.deliver("snapshot", b(4));
    stream.deliver("output", b(5));
    expect(writes.slice(3).map((w) => [...w.bytes])).toEqual([[4], [5]]);
  });
});
