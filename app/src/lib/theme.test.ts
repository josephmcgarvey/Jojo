import { describe, expect, it, vi } from "vitest";
import neonGlass from "../themes/neon-glass.json";
import { BASE_COLOR_PRESETS, contrast, effectiveAccent, parseHex, recolorTheme } from "./palette";
import { THEMES, applyTheme, resolveTheme, themeById, type Theme } from "./theme";

const REQUIRED_TOKENS = ["bg", "bg-glow", "side", "bar", "pane", "line", "sel", "fg", "fg2", "muted", "accent", "working", "needs-you", "done", "error", "idle", "font-ui", "font-mono", "brand-gradient", "radius", "radius-sm", "danger", "scrim", "menu", "shadow", "glass-side", "glass-pane", "focus-ring", "focus-border", "tab-glow", "sel-gradient", "pill", "radius-lg"];

/** Text contrast floors: fg and fg2 are body text, muted is secondary text (WCAG AA 4.5:1). */
function textContrast(theme: Theme): Record<string, number> {
  const bg = parseHex(theme.tokens.bg)!;
  const terminalBg = parseHex(theme.terminal.background!)!;
  return {
    fg: contrast(parseHex(theme.tokens.fg)!, bg),
    fg2: contrast(parseHex(theme.tokens.fg2)!, bg),
    muted: contrast(parseHex(theme.tokens.muted)!, bg),
    terminal: contrast(parseHex(theme.terminal.foreground!)!, terminalBg),
    // The primary button draws bg-colored text on the accent.
    accentButton: contrast(parseHex(theme.tokens.bg)!, parseHex(theme.tokens.accent)!),
  };
}

function expectReadable(theme: Theme, label: string): void {
  const c = textContrast(theme);
  expect(c.fg, `${label} fg`).toBeGreaterThanOrEqual(7);
  expect(c.fg2, `${label} fg2`).toBeGreaterThanOrEqual(7);
  expect(c.muted, `${label} muted`).toBeGreaterThanOrEqual(4.5);
  expect(c.terminal, `${label} terminal`).toBeGreaterThanOrEqual(7);
  expect(c.accentButton, `${label} accent button`).toBeGreaterThanOrEqual(4.5);
}

describe("theme", () => {
  it("writes every token as a --jojo- custom property, and color-scheme", () => {
    const setProperty = vi.fn();
    applyTheme(neonGlass as Theme, { style: { setProperty } });
    expect(setProperty).toHaveBeenCalledWith("--jojo-bg", "#060816");
    expect(setProperty).toHaveBeenCalledWith("color-scheme", "dark");
    expect(setProperty).toHaveBeenCalledTimes(Object.keys(neonGlass.tokens).length + 1);
  });

  it.each(Object.entries(THEMES))("%s defines the tokens and terminal colors the UI uses", (_id, theme) => {
    for (const name of REQUIRED_TOKENS) expect(theme.tokens, name).toHaveProperty(name);
    for (const key of ["background", "foreground", "cursor", "selectionBackground", "black", "brightWhite"]) {
      expect(theme.terminal, key).toHaveProperty(key);
    }
    expect(theme.tokens.accent).toBe(theme.accent);
    expect(effectiveAccent(theme, theme.accent)).toBe(theme.accent);
  });

  it.each(Object.entries(THEMES))("%s is readable with its own and every preset base color", (id, theme) => {
    expectReadable(theme, id);
    for (const preset of BASE_COLOR_PRESETS) expectReadable(recolorTheme(theme, preset.color), `${id} ${preset.name}`);
    for (let hue = 0; hue < 360; hue += 30) {
      expectReadable(recolorTheme(theme, hslHex(hue)), `${id} hue ${hue}`);
    }
  });

  it("falls back to Neon Glass for an unknown theme id", () => {
    expect(themeById("removed-theme")).toBe(THEMES["neon-glass"]);
  });

  it("applies the interface font and base color", () => {
    const theme = resolveTheme({ theme: "neon-glass", accentColor: "#5aa2ff", uiFontFamily: "Inter" });
    expect(theme.tokens["font-ui"]).toBe('"Inter", "Segoe UI", sans-serif');
    expect(theme.tokens.accent).toBe("#5aa2ff");
  });
});

/** A saturated mid-light color at `hue` degrees, as hex. */
function hslHex(hue: number): string {
  const f = (n: number) => {
    const k = (n + hue / 30) % 12;
    const v = 0.7 - 0.3 * Math.max(-1, Math.min(k - 3, 9 - k, 1));
    return Math.round(v * 255).toString(16).padStart(2, "0");
  };
  return `#${f(0)}${f(8)}${f(4)}`;
}
