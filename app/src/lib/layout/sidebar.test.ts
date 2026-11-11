import { describe, expect, it } from "vitest";
import { clampSidebarWidth, DEFAULT_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH, MIN_SIDEBAR_WIDTH } from "./sidebar";

describe("clampSidebarWidth", () => {
  it("keeps the width between the minimum and the maximum, rounded", () => {
    expect(clampSidebarWidth(300.6)).toBe(301);
    expect(clampSidebarWidth(50)).toBe(MIN_SIDEBAR_WIDTH);
    expect(clampSidebarWidth(5000)).toBe(MAX_SIDEBAR_WIDTH);
  });

  it("uses the default for a missing or invalid value", () => {
    expect(clampSidebarWidth(undefined)).toBe(DEFAULT_SIDEBAR_WIDTH);
    expect(clampSidebarWidth(null)).toBe(DEFAULT_SIDEBAR_WIDTH);
    expect(clampSidebarWidth(Number.NaN)).toBe(DEFAULT_SIDEBAR_WIDTH);
  });
});
