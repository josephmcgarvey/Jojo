import { describe, expect, it } from "vitest";
import { contextMenuTarget } from "./mouse";

describe("contextMenuTarget", () => {
  it("opens Jojo's menu on a plain right-click when mouse reporting is off", () => {
    expect(contextMenuTarget(false, "none")).toBe("jojo");
  });

  it("sends a plain right-click to the program when mouse reporting is on", () => {
    for (const mode of ["x10", "vt200", "drag", "any"] as const) expect(contextMenuTarget(false, mode), mode).toBe("program");
  });

  it("always opens Jojo's menu on Shift+right-click", () => {
    for (const mode of ["none", "x10", "vt200", "drag", "any"] as const) expect(contextMenuTarget(true, mode), mode).toBe("jojo");
  });
});
