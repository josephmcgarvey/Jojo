import { describe, expect, it } from "vitest";
import { CHIME_INTERVAL_MS, chimeFor } from "./chime";

describe("chimeFor", () => {
  it("chimes when a session newly finishes, needs you or fails", () => {
    expect(chimeFor("working", "done", true, undefined, 0)).toBe("done");
    expect(chimeFor("working", "needs_you", true, undefined, 0)).toBe("needs_you");
    expect(chimeFor("working", "error", true, undefined, 0)).toBe("error");
  });

  it("stays quiet for other states, repeats, a disabled setting and within the interval", () => {
    expect(chimeFor("idle", "working", true, undefined, 0)).toBeNull();
    expect(chimeFor("done", "done", true, undefined, 0)).toBeNull();
    expect(chimeFor("working", "done", false, undefined, 0)).toBeNull();
    expect(chimeFor("working", "done", true, 1_000, 1_000 + CHIME_INTERVAL_MS - 1)).toBeNull();
    expect(chimeFor("working", "done", true, 1_000, 1_000 + CHIME_INTERVAL_MS)).toBe("done");
  });
});
