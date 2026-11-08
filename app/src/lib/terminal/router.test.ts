import { describe, expect, it, vi } from "vitest";
import { createRouter } from "./router";

describe("router", () => {
  it("delivers data frames, re-attaches on fell-behind and reconnect, reports attach failures, and unregisters", () => {
    const router = createRouter();
    const pane = { deliver: vi.fn(), reattach: vi.fn(), attachFailed: vi.fn() };
    const unregister = router.register("S1", pane);
    router.deliver({ kind: "output", sessionId: "S1", bytes: new Uint8Array([1]) });
    router.deliver({ kind: "output", sessionId: "S2", bytes: new Uint8Array([2]) });
    expect(pane.deliver).toHaveBeenCalledTimes(1);
    router.fellBehind("S1");
    expect(pane.reattach).toHaveBeenLastCalledWith(false);
    router.reattachAll();
    expect(pane.reattach).toHaveBeenLastCalledWith(true);
    router.reattachAll((id) => id === "OTHER");
    expect(pane.reattach).toHaveBeenCalledTimes(2);
    router.attachFailed("S1");
    expect(pane.attachFailed).toHaveBeenCalledTimes(1);
    unregister();
    router.deliver({ kind: "output", sessionId: "S1", bytes: new Uint8Array([3]) });
    expect(pane.deliver).toHaveBeenCalledTimes(1);
  });
});
