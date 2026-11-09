import { describe, expect, it } from "vitest";
import neonGlass from "../themes/neon-glass.json";
import { contrast, effectiveAccent, parseHex, recolorTheme } from "./palette";
import type { Theme } from "./theme";

const theme = neonGlass as Theme;

describe("palette", () => {
  it("returns the theme unchanged for its own accent, an empty value or an invalid one", () => {
    expect(recolorTheme(theme, "")).toBe(theme);
    expect(recolorTheme(theme, "#9B8CFF")).toBe(theme);
    expect(recolorTheme(theme, "not a color")).toBe(theme);
  });

  it("replaces the accent at every alpha and keeps the state colors", () => {
    const blue = recolorTheme(theme, "#5aa2ff");
    expect(blue.tokens.accent).toBe("#5aa2ff");
    expect(blue.tokens["accent-glow"]).toBe("rgba(90, 162, 255, 0.55)");
    expect(blue.tokens["tab-glow"]).toBe("0 0 12px rgba(90, 162, 255, 0.6)");
    expect(blue.terminal.cursor).toBe("#5aa2ff");
    for (const name of ["working", "needs-you", "done", "error", "danger", "banner"]) expect(blue.tokens[name], name).toBe(theme.tokens[name]);
    expect(blue.terminal.red).toBe(theme.terminal.red);
    expect(blue.tokens.shadow).toBe(theme.tokens.shadow);
  });

  it("turns tinted neutrals to the new hue", () => {
    const green = recolorTheme(theme, "#5fd38a");
    const bg = parseHex(green.tokens.bg)!;
    // Neon Glass's #060816 is blue; under green the green channel leads.
    expect(bg.g).toBeGreaterThan(bg.b);
    expect(green.tokens["bg-glow"]).toContain(green.tokens.bg);
  });

  it("lightens a base color that is too dark to read on the background", () => {
    // #1a1050 against #060816 is about 1.3:1.
    const accent = effectiveAccent(theme, "#1a1050");
    expect(accent).not.toBe("#1a1050");
    expect(contrast(parseHex(accent)!, parseHex(theme.tokens.bg)!)).toBeGreaterThanOrEqual(4.5);
    expect(contrast(parseHex(accent)!, parseHex(theme.tokens.bg)!)).toBeLessThan(4.7);
  });
});
