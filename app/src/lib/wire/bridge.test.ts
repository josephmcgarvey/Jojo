import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: class {} }));

import { dispatchEnvelope } from "./bridge";
import { encodeControl } from "./frame";

describe("dispatchEnvelope", () => {
  it("routes frames and statuses", () => {
    const onFrame = vi.fn();
    const onStatus = vi.fn();
    const local = [5, ...new TextEncoder().encode("local")];
    dispatchEnvelope(new Uint8Array([0, ...local, ...encodeControl({ type: "ListProjects" })]), { onFrame, onStatus });
    expect(onFrame).toHaveBeenCalledWith("local", { kind: "control", msg: { type: "ListProjects" } });
    const status = new TextEncoder().encode('{"state":"elevated"}');
    const remote = [3, ...new TextEncoder().encode("01A")];
    dispatchEnvelope(new Uint8Array([1, ...remote, ...status]), { onFrame, onStatus });
    expect(onStatus).toHaveBeenCalledWith("01A", { state: "elevated" });
  });
});
