import { describe, expect, it } from "vitest";
import { contextMeter, contextTooltip, formatTokens } from "./context";

describe("context meter", () => {
  it("formats token counts", () => {
    expect(formatTokens(999)).toBe("999");
    expect(formatTokens(150_004)).toBe("150k");
    expect(formatTokens(1_000_000)).toBe("1M");
    expect(formatTokens(1_250_000)).toBe("1.3M");
  });

  it("turns amber at 70% and red at 85%", () => {
    expect(contextMeter(100_000, 200_000)).toEqual({ fraction: 0.5, level: "ok", text: "100k of 200k tokens (50%)" });
    expect(contextMeter(140_000, 200_000)?.level).toBe("warn");
    expect(contextMeter(170_000, 200_000)?.level).toBe("full");
    expect(contextMeter(460_298, 1_000_000)?.level).toBe("ok");
    expect(contextMeter(250_000, 200_000)?.fraction).toBe(1);
  });

  it("shows nothing without usage", () => {
    expect(contextMeter(null, null)).toBeNull();
    expect(contextMeter(10, 0)).toBeNull();
  });

  it("advises a /compact when it is filling up", () => {
    expect(contextTooltip(contextMeter(170_000, 200_000)!)).toBe("Context: 170k of 200k tokens (85%). Run /compact now; Claude Code compacts on its own when the window is nearly full.");
  });
});
