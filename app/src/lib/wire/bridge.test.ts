import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: class {} }));

import { dispatchEnvelope } from "./bridge";
import { encodeControl } from "./frame";

describe("dispatchEnvelope", () => {
  it("routes frames and statuses", () => {
    const onFrame = vi.fn();
    const onStatus = vi.fn();
    dispatchEnvelope(new Uint8Array([0, ...encodeControl({ type: "ListProjects" })]), { onFrame, onStatus });
    expect(onFrame).toHaveBeenCalledWith({ kind: "control", msg: { type: "ListProjects" } });
    const status = new TextEncoder().encode('{"state":"elevated"}');
    dispatchEnvelope(new Uint8Array([1, ...status]), { onFrame, onStatus });
    expect(onStatus).toHaveBeenCalledWith({ state: "elevated" });
  });
});
