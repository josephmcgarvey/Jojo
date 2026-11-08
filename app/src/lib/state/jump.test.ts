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

  it("moves from the only needs_you session to a done one, and stays when nothing else needs attention", () => {
    expect(nextAttentionSession([s("A", "needs_you"), s("B", "done")], "A")).toBe("B");
    expect(nextAttentionSession([s("A", "needs_you"), s("B", "idle")], "A")).toBe("A");
  });
});
