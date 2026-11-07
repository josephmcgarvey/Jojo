// Frame bodies as the backend relays them: [u8 type][payload], without the
// u32 length prefix used on the daemon pipe (spec section 6.1).
import type { ControlMsg } from "../protocol/ControlMsg";
import { bytesToUlid, ulidToBytes } from "./ulid";

export const FRAME_CONTROL = 1;
export const FRAME_INPUT = 2;
export const FRAME_OUTPUT = 3;
export const FRAME_SNAPSHOT = 4;

export type Frame =
  | { kind: "control"; msg: ControlMsg }
  | { kind: "output" | "snapshot" | "input"; sessionId: string; bytes: Uint8Array };

const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function decodeFrameBody(body: Uint8Array): Frame {
  const type = body[0];
  if (type === FRAME_CONTROL) {
    return { kind: "control", msg: JSON.parse(decoder.decode(body.subarray(1))) as ControlMsg };
  }
  const kind = type === FRAME_OUTPUT ? "output" : type === FRAME_SNAPSHOT ? "snapshot" : type === FRAME_INPUT ? "input" : null;
  if (kind === null) throw new Error(`unknown frame type ${type}`);
  if (body.length < 17) throw new Error("data frame shorter than its 16-byte session id");
  return { kind, sessionId: bytesToUlid(body.subarray(1, 17)), bytes: body.slice(17) };
}

export function encodeControl(msg: ControlMsg): Uint8Array {
  const json = encoder.encode(JSON.stringify(msg));
  const out = new Uint8Array(1 + json.length);
  out[0] = FRAME_CONTROL;
  out.set(json, 1);
  return out;
}

export function encodeInput(sessionId: string, bytes: Uint8Array): Uint8Array {
  const out = new Uint8Array(17 + bytes.length);
  out[0] = FRAME_INPUT;
  out.set(ulidToBytes(sessionId), 1);
  out.set(bytes, 17);
  return out;
}
