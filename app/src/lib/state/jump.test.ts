import { describe, expect, it } from "vitest";
import { nextAttentionSession } from "./jump";

const s = (id: string, state: "idle" | "working" | "needs_you" | "done") => ({ id, state });

describe("nextAttentionSession", () => {
  it("prefers the next needs_you session after the focused one, wrapping around", () => {
    const sessions = [s("A", "needs_you"), s("B", "done"), s("C", "idle"), s("D", "needs_you")];
    expect(nextAttentionSession(sessions, "B")).toBe("D");
    expect(nextAttentionSession(sessions, "D")).toBe("A");
    expect(nextAttentionSession(sessions, null)).toBe("A");
  });

  it("falls back to done sessions, then to nothing", () => {
    expect(nextAttentionSession([s("A", "idle"), s("B", "done"), s("C", "working")], "C")).toBe("B");
    expect(nextAttentionSession([s("A", "idle"), s("B", "working")], "A")).toBeNull();
  });
});
