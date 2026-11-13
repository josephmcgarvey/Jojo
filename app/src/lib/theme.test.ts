import { describe, expect, it, vi } from "vitest";
import neonGlass from "../themes/neon-glass.json";
import { BASE_COLOR_PRESETS, contrast, effectiveAccent, parseHex, recolorTheme } from "./palette";
import { THEMES, applyTheme, resolveTheme, themeById, type Theme } from "./theme";

const REQUIRED_TOKENS = ["bg", "bg-glow", "side", "bar", "pane", "line", "sel", "fg", "fg2", "muted", "accent", "working", "needs-you", "done", "error", "idle", "font-ui", "font-mono", "brand-gradient", "radius", "radius-sm", "danger", "scrim", "menu", "shadow", "glass-side", "glass-pane", "focus-ring", "modal-glow","focus-border", "tab-glow", "sel-gradient", "pill", "radius-lg", "side-blur", "pane-blur", "side-margin", "side-radius", "bw", "dot-radius", "text-glow", "panel-shadow"];

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
    const setAttribute = vi.fn();
    applyTheme(neonGlass as Theme, { style: { setProperty }, setAttribute });
    expect(setProperty).toHaveBeenCalledWith("--jojo-bg", "#060816");
    expect(setProperty).toHaveBeenCalledWith("color-scheme", "dark");
    expect(setProperty).toHaveBeenCalledTimes(Object.keys(neonGlass.tokens).length + 1);
    expect(setAttribute).toHaveBeenCalledWith("data-effects", "");
  });

  it("writes Prism's rainbow effect to data-effects", () => {
    const setAttribute = vi.fn();
    applyTheme(THEMES.prism, { style: { setProperty: vi.fn() }, setAttribute });
    expect(setAttribute).toHaveBeenCalledWith("data-effects", "rainbow");
  });

  it("keeps Prism's rainbow colors under a base color", () => {
    const blue = resolveTheme({ theme: "prism", accentColor: "#5aa2ff" });
    for (const name of ["focus-ring", "focus-border", "brand-gradient", "sel-gradient", "tab-glow"]) {
      expect(blue.tokens[name], name).toBe(THEMES.prism.tokens[name]);
      expect(blue.tokens[name], name).toContain("var(--jojo-hue)");
    }
    // The background glow keeps its hue-driven glows; its final hex layer is the background, which the base color tints.
    expect(blue.tokens["bg-glow"]).toBe(THEMES.prism.tokens["bg-glow"].replace(THEMES.prism.tokens.bg, blue.tokens.bg));
    expect(blue.tokens.accent).toBe("#5aa2ff");
    expect(blue.effects).toEqual(["rainbow"]);
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
    expect(theme.tokens["font-ui"]).toBe('"Inter", "Segoe UI", system-ui, sans-serif');
    expect(resolveTheme({ uiFontFamily: "system-ui" }).tokens["font-ui"]).toBe('system-ui, "Segoe UI", system-ui, sans-serif');
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
