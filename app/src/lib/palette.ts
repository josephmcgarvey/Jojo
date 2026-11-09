// Recolors a theme from one base color (the Settings ▸ Base color control).
//
// A theme JSON is drawn around its own accent (`theme.accent`, for example
// Neon Glass's #9b8cff). Every color literal in its tokens and terminal
// colors is one of three kinds, and each is rewritten differently:
//
// 1. The accent itself, at any alpha (rgba(155, 140, 255, 0.55) is the
//    accent glow). It becomes the new base color at the same alpha.
// 2. A tinted neutral: a color whose hue is within NEUTRAL_HUE_RANGE degrees
//    of the accent's hue (Neon Glass's backgrounds, lines and text are blue
//    violet, hue 226 to 248). Its hue turns by the same angle as the accent's
//    and its saturation shrinks when the new base color is less saturated
//    (a gray base gives gray panels); its lightness is kept, then corrected
//    so its luminance, and with it its contrast, stays the same.
// 3. Everything else (the state colors working, needs you, done and error,
//    the mint and cyan glows, black shadows). It is kept, so a state color
//    keeps its meaning under every base color.
//
// The base color must reach ACCENT_CONTRAST against the theme's background
// (the primary button draws background-colored text on it, and focus rings
// and the selected tab use it). A base color below that is lightened on a
// dark theme or darkened on a light one, by the smallest lightness change
// that reaches it; its hue and saturation are kept.
import type { ITheme } from "@xterm/xterm";
import type { Theme } from "./theme";

/** WCAG AA contrast for normal text, required of the base color against the background. */
const ACCENT_CONTRAST = 4.5;

/** Hue distance (degrees) within which a color counts as a tinted neutral. */
const NEUTRAL_HUE_RANGE = 40;

/** State tokens that are never recolored, whatever their hue. */
const FIXED_TOKENS = new Set(["working", "needs-you", "needs-you-glow", "needs-you-soft", "done", "error", "danger", "banner"]);

/** xterm.js colors that follow the base color; the 16 ANSI colors keep their meaning. */
const TINTED_TERMINAL_KEYS: (keyof ITheme)[] = ["background", "foreground", "cursor", "cursorAccent", "selectionBackground", "black", "white", "brightBlack", "brightWhite"];

/** Base color presets shown as swatches in Settings. */
export const BASE_COLOR_PRESETS: { name: string; color: string }[] = [
  { name: "Violet", color: "#9b8cff" },
  { name: "Indigo", color: "#7b8cff" },
  { name: "Blue", color: "#5aa2ff" },
  { name: "Teal", color: "#2fc4c4" },
  { name: "Green", color: "#5fd38a" },
  { name: "Rose", color: "#ff6fa3" },
  { name: "Orange", color: "#ff8a4c" },
  { name: "Slate", color: "#a0a8c8" },
];

interface Rgb {
  r: number;
  g: number;
  b: number;
}
interface Hsl {
  h: number;
  s: number;
  l: number;
}

/** `#rgb` or `#rrggbb` (any case) as 0-255 channels; null for anything else. */
export function parseHex(value: string): Rgb | null {
  const m = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(value.trim());
  if (!m) return null;
  const hex = m[1].length === 3 ? [...m[1]].map((c) => c + c).join("") : m[1];
  return { r: parseInt(hex.slice(0, 2), 16), g: parseInt(hex.slice(2, 4), 16), b: parseInt(hex.slice(4, 6), 16) };
}

export function toHex({ r, g, b }: Rgb): string {
  return `#${[r, g, b].map((c) => Math.round(c).toString(16).padStart(2, "0")).join("")}`;
}

function toHsl({ r, g, b }: Rgb): Hsl {
  const [rn, gn, bn] = [r / 255, g / 255, b / 255];
  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const l = (max + min) / 2;
  const d = max - min;
  if (d === 0) return { h: 0, s: 0, l };
  const s = d / (1 - Math.abs(2 * l - 1));
  let h: number;
  if (max === rn) h = ((gn - bn) / d) % 6;
  else if (max === gn) h = (bn - rn) / d + 2;
  else h = (rn - gn) / d + 4;
  return { h: (h * 60 + 360) % 360, s, l };
}

function fromHsl({ h, s, l }: Hsl): Rgb {
  const c = (1 - Math.abs(2 * l - 1)) * s;
  const hp = (((h % 360) + 360) % 360) / 60;
  const x = c * (1 - Math.abs((hp % 2) - 1));
  const [r1, g1, b1] = hp < 1 ? [c, x, 0] : hp < 2 ? [x, c, 0] : hp < 3 ? [0, c, x] : hp < 4 ? [0, x, c] : hp < 5 ? [x, 0, c] : [c, 0, x];
  const m = l - c / 2;
  return { r: Math.round((r1 + m) * 255), g: Math.round((g1 + m) * 255), b: Math.round((b1 + m) * 255) };
}

/** WCAG 2 relative luminance (0 black to 1 white). */
export function luminance({ r, g, b }: Rgb): number {
  const lin = (c: number) => {
    const v = c / 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

/** WCAG 2 contrast ratio between two opaque colors (1 to 21). */
export function contrast(a: Rgb, b: Rgb): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

function hueDistance(a: number, b: number): number {
  const d = Math.abs(a - b) % 360;
  return d > 180 ? 360 - d : d;
}

/** The color with HSL lightness chosen so its luminance is `target` (hue and saturation kept). */
function withLuminance(color: Hsl, target: number): Rgb {
  let lo = 0;
  let hi = 1;
  for (let i = 0; i < 24; i++) {
    const mid = (lo + hi) / 2;
    if (luminance(fromHsl({ ...color, l: mid })) < target) lo = mid;
    else hi = mid;
  }
  return fromHsl({ ...color, l: (lo + hi) / 2 });
}

/**
 * The base color a theme actually uses for `requested`: the requested hex
 * (lowercase), lightened or darkened until it reaches ACCENT_CONTRAST
 * against the theme's background, or the theme's own accent when
 * `requested` is empty or not a hex color.
 */
export function effectiveAccent(theme: Theme, requested: string | null | undefined): string {
  const rgb = requested ? parseHex(requested) : null;
  if (!rgb) return theme.accent;
  const bg = parseHex(theme.tokens.bg)!;
  if (contrast(rgb, bg) >= ACCENT_CONTRAST) return toHex(rgb);
  const hsl = toHsl(rgb);
  // Search the lightness between the requested one and the far end (white
  // on a dark theme, black on a light one) for the nearest that passes. The
  // search aims 0.05 above the floor, because rounding the channels to whole
  // numbers can lower the contrast by up to about 0.03.
  const target = ACCENT_CONTRAST + 0.05;
  let near = hsl.l;
  let far = theme.mode === "dark" ? 1 : 0;
  for (let i = 0; i < 24; i++) {
    const mid = (near + far) / 2;
    if (contrast(fromHsl({ ...hsl, l: mid }), bg) >= target) far = mid;
    else near = mid;
  }
  return toHex(fromHsl({ ...hsl, l: far }));
}

const COLOR = /#[0-9a-f]{6}\b|#[0-9a-f]{3}\b|rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*([\d.]+)\s*)?\)/gi;

/** Rewrites every color literal in a CSS value with `map`, keeping its notation and alpha. */
function mapColors(value: string, map: (color: Rgb) => Rgb): string {
  return value.replace(COLOR, (match, r?: string, g?: string, b?: string, a?: string) => {
    if (match.startsWith("#")) {
      const rgb = parseHex(match);
      return rgb ? toHex(map(rgb)) : match;
    }
    const out = map({ r: Number(r), g: Number(g), b: Number(b) });
    return a === undefined ? `rgb(${out.r}, ${out.g}, ${out.b})` : `rgba(${out.r}, ${out.g}, ${out.b}, ${a})`;
  });
}

/** The theme redrawn around `requested` (see the file comment); the theme itself when it is its own accent. */
export function recolorTheme(theme: Theme, requested: string | null | undefined): Theme {
  const accentHex = effectiveAccent(theme, requested);
  const from = parseHex(theme.accent)!;
  const to = parseHex(accentHex)!;
  if (toHex(from) === toHex(to)) return theme;
  const fromHsl_ = toHsl(from);
  const toHsl_ = toHsl(to);
  const rotate = toHsl_.h - fromHsl_.h;
  const saturationScale = fromHsl_.s === 0 ? 1 : Math.min(1, toHsl_.s / fromHsl_.s);
  const fromKey = toHex(from);

  const map = (color: Rgb): Rgb => {
    if (toHex(color) === fromKey) return to;
    const hsl = toHsl(color);
    if (hsl.s === 0 || hueDistance(hsl.h, fromHsl_.h) > NEUTRAL_HUE_RANGE) return color;
    return withLuminance({ h: hsl.h + rotate, s: hsl.s * saturationScale, l: hsl.l }, luminance(color));
  };

  const tokens = Object.fromEntries(Object.entries(theme.tokens).map(([name, value]) => [name, FIXED_TOKENS.has(name) ? value : mapColors(value, map)]));
  const terminal: ITheme = { ...theme.terminal };
  for (const key of TINTED_TERMINAL_KEYS) {
    const value = terminal[key];
    if (typeof value === "string") (terminal as Record<string, unknown>)[key] = mapColors(value, map);
  }
  return { ...theme, accent: accentHex, tokens, terminal };
}
