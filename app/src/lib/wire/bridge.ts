// The link to the Rust backend: one Channel for everything coming in and a
// raw-body invoke for frames going out. Incoming envelope (bridge.rs): byte
// 0 is the kind (0 = daemon frame body, 1 = ConnectionStatus JSON), byte 1
// the length of the machine id, then the machine id, then the body.
import { Channel, invoke } from "@tauri-apps/api/core";
import type { ConnectionStatus } from "../backend/ConnectionStatus";
import type { ControlMsg } from "../protocol/ControlMsg";
import { decodeFrameBody, encodeControl, encodeInput, type Frame } from "./frame";

/** The machine id of This PC (bridge.rs LOCAL_MACHINE). */
export const LOCAL_MACHINE = "local";

export interface BridgeHandlers {
  onStatus(machine: string, status: ConnectionStatus): void;
  onFrame(machine: string, frame: Frame): void;
}

const decoder = new TextDecoder();

export function dispatchEnvelope(bytes: Uint8Array, handlers: BridgeHandlers): void {
  const idLength = bytes[1];
  const machine = decoder.decode(bytes.subarray(2, 2 + idLength));
  const body = bytes.subarray(2 + idLength);
  if (bytes[0] === 0) handlers.onFrame(machine, decodeFrameBody(body));
  else if (bytes[0] === 1) handlers.onStatus(machine, JSON.parse(decoder.decode(body)) as ConnectionStatus);
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

/** Sends a control message to `machine` (This PC by default). */
export function sendControl(msg: ControlMsg, machine: string = LOCAL_MACHINE): Promise<void> {
  return invoke("send_frame", encodeControl(msg), { headers: { "jojo-machine": machine } });
}

/** Sends keystrokes for a session on `machine` (This PC by default). */
export function sendInput(sessionId: string, bytes: Uint8Array, machine: string = LOCAL_MACHINE): Promise<void> {
  return invoke("send_frame", encodeInput(sessionId, bytes), { headers: { "jojo-machine": machine } });
}
