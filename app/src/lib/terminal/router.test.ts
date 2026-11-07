import { describe, expect, it, vi } from "vitest";
import { createRouter } from "./router";

describe("router", () => {
  it("delivers data frames, re-attaches on fell-behind and reconnect, and unregisters", () => {
    const router = createRouter();
    const pane = { deliver: vi.fn(), reattach: vi.fn() };
    const unregister = router.register("S1", pane);
    router.deliver({ kind: "output", sessionId: "S1", bytes: new Uint8Array([1]) });
    router.deliver({ kind: "output", sessionId: "S2", bytes: new Uint8Array([2]) });
    expect(pane.deliver).toHaveBeenCalledTimes(1);
    router.fellBehind("S1");
    router.reattachAll();
    expect(pane.reattach).toHaveBeenCalledTimes(2);
    unregister();
    router.deliver({ kind: "output", sessionId: "S1", bytes: new Uint8Array([3]) });
    expect(pane.deliver).toHaveBeenCalledTimes(1);
  });
});
