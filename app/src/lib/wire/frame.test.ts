import { describe, expect, it } from "vitest";
import { decodeFrameBody, encodeControl, encodeInput, FRAME_CONTROL, FRAME_INPUT, FRAME_OUTPUT } from "./frame";
import { ulidToBytes } from "./ulid";

const ID = "01J00000000000000000000000";

describe("frame", () => {
  it("encodes control frames as a type byte and JSON", () => {
    const body = encodeControl({ type: "ListSessions" });
    expect(body[0]).toBe(FRAME_CONTROL);
    expect(new TextDecoder().decode(body.subarray(1))).toBe('{"type":"ListSessions"}');
  });

  it("decodes control frames", () => {
    const body = encodeControl({ type: "SessionRemoved", session_id: ID });
    expect(decodeFrameBody(body)).toEqual({ kind: "control", msg: { type: "SessionRemoved", session_id: ID } });
  });

  it("encodes input as type, 16-byte id, bytes", () => {
    const body = encodeInput(ID, new Uint8Array([104, 105]));
    expect(body[0]).toBe(FRAME_INPUT);
    expect(body.subarray(1, 17)).toEqual(ulidToBytes(ID));
    expect([...body.subarray(17)]).toEqual([104, 105]);
  });

  it("decodes output frames", () => {
    const body = new Uint8Array([FRAME_OUTPUT, ...ulidToBytes(ID), 7, 8]);
    const frame = decodeFrameBody(body);
    expect(frame).toEqual({ kind: "output", sessionId: ID, bytes: new Uint8Array([7, 8]) });
  });

  it("rejects unknown and short frames", () => {
    expect(() => decodeFrameBody(new Uint8Array([99]))).toThrow();
    expect(() => decodeFrameBody(new Uint8Array([FRAME_OUTPUT, 1, 2]))).toThrow();
  });
});
