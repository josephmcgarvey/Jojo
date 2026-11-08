import { describe, expect, it } from "vitest";
import { AttachGate } from "./attachGate";

const b = (n: number) => new Uint8Array([n]);

describe("AttachGate", () => {
  it("drops output until the snapshot of the outstanding attach arrives", () => {
    const gate = new AttachGate();
    expect(gate.begin()).toBe(true);
    expect(gate.accept("output", b(1))).toBeNull();
    expect(gate.accept("snapshot", b(2))).toEqual(b(2));
    expect(gate.accept("output", b(3))).toEqual(b(3));
  });

  it("passes output when no attach is outstanding", () => {
    expect(new AttachGate().accept("output", b(4))).toEqual(b(4));
  });

  it("refuses a second attach while one is outstanding", () => {
    const gate = new AttachGate();
    expect(gate.begin()).toBe(true);
    expect(gate.begin()).toBe(false);
    expect(gate.outstanding).toBe(true);
    gate.accept("snapshot", b(1));
    expect(gate.outstanding).toBe(false);
    expect(gate.begin()).toBe(true);
  });

  it("releases an attach that will never be answered", () => {
    const gate = new AttachGate();
    gate.begin();
    gate.abandon();
    expect(gate.outstanding).toBe(false);
    expect(gate.accept("output", b(5))).toEqual(b(5));
    expect(gate.begin()).toBe(true);
  });

  it("writes a snapshot nobody is waiting for without counting it", () => {
    const gate = new AttachGate();
    expect(gate.accept("snapshot", b(6))).toEqual(b(6));
    expect(gate.begin()).toBe(true);
    expect(gate.accept("output", b(7))).toBeNull();
  });
});
