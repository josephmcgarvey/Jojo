// The link to the Rust backend: one Channel for everything coming in
// (envelope byte 0 = daemon frame body, 1 = ConnectionStatus JSON) and a
// raw-body invoke for frames going out.
import { Channel, invoke } from "@tauri-apps/api/core";
import type { ConnectionStatus } from "../backend/ConnectionStatus";
import type { ControlMsg } from "../protocol/ControlMsg";
import { decodeFrameBody, encodeControl, encodeInput, type Frame } from "./frame";

export interface BridgeHandlers {
  onStatus(status: ConnectionStatus): void;
  onFrame(frame: Frame): void;
}

const decoder = new TextDecoder();

export function dispatchEnvelope(bytes: Uint8Array, handlers: BridgeHandlers): void {
  const body = bytes.subarray(1);
  if (bytes[0] === 0) handlers.onFrame(decodeFrameBody(body));
  else if (bytes[0] === 1) handlers.onStatus(JSON.parse(decoder.decode(body)) as ConnectionStatus);
}

function toBytes(data: unknown): Uint8Array {
  if (data instanceof ArrayBuffer) return new Uint8Array(data);
  if (data instanceof Uint8Array) return data;
  return Uint8Array.from(data as number[]);
}

export async function subscribe(handlers: BridgeHandlers): Promise<void> {
  const channel = new Channel<unknown>();
  channel.onmessage = (data) => dispatchEnvelope(toBytes(data), handlers);
  await invoke("subscribe", { events: channel });
}

export function sendControl(msg: ControlMsg): Promise<void> {
  return invoke("send_frame", encodeControl(msg));
}

export function sendInput(sessionId: string, bytes: Uint8Array): Promise<void> {
  return invoke("send_frame", encodeInput(sessionId, bytes));
}
