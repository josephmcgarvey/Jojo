// A theme is one JSON file: `tokens` become --jojo-* custom properties on
// :root, `terminal` is the xterm.js palette (spec section 9.9). `accent` is
// the base color the theme is drawn around (see palette.ts).
import type { ITheme } from "@xterm/xterm";
import contrast from "../themes/contrast.json";
import daylight from "../themes/daylight.json";
import graphite from "../themes/graphite.json";
import neonGlass from "../themes/neon-glass.json";
import phosphor from "../themes/phosphor.json";
import porcelain from "../themes/porcelain.json";
import prism from "../themes/prism.json";
import { uiFontStack } from "./fonts";
import { recolorTheme } from "./palette";

export interface Theme {
  name: string;
  mode: "dark" | "light";
  accent: string;
  tokens: Record<string, string>;
  terminal: ITheme;
  /**
   * Animated effects the theme switches on, written to the root element's
   * data-effects attribute for app.css. "rainbow" animates --jojo-hue, which
   * the theme's tokens use in hsl() colors.
   */
  effects?: string[];
}

/** The themes in Settings ▸ Theme, by the id stored in settings.json. */
export const THEMES: Record<string, Theme> = {
  "neon-glass": neonGlass as Theme,
  prism: prism as Theme,
  daylight: daylight as Theme,
  graphite: graphite as Theme,
  phosphor: phosphor as Theme,
  porcelain: porcelain as Theme,
  contrast: contrast as Theme,
};

export const DEFAULT_THEME_ID = "neon-glass";

/** The theme for an id; an unknown id (a removed theme, a hand edit) gives Neon Glass. */
export function themeById(id: string | null | undefined): Theme {
  return THEMES[id ?? DEFAULT_THEME_ID] ?? THEMES[DEFAULT_THEME_ID];
}

/** The appearance settings a theme is built from. */
export interface Appearance {
  theme: string;
  accentColor: string;
  uiFontFamily: string;
}

/** The theme for the appearance settings: recolored around the base color, with the interface font. */
export function resolveTheme(appearance: Partial<Appearance> | null | undefined): Theme {
  const theme = recolorTheme(themeById(appearance?.theme), appearance?.accentColor);
  if (!appearance?.uiFontFamily) return theme;
  return { ...theme, tokens: { ...theme.tokens, "font-ui": uiFontStack(appearance.uiFontFamily) } };
}

interface StyleTarget {
  style: { setProperty(name: string, value: string): void };
  setAttribute(name: string, value: string): void;
}

/**
 * Writes the tokens as --jojo-* properties, sets color-scheme (which colors
 * scrollbars and native controls), and writes the theme's effects to
 * data-effects (empty for a theme without any).
 */
export function applyTheme(theme: Theme, root: StyleTarget = document.documentElement): void {
  for (const [name, value] of Object.entries(theme.tokens)) {
    root.style.setProperty(`--jojo-${name}`, value);
  }
  root.style.setProperty("color-scheme", theme.mode);
  root.setAttribute("data-effects", (theme.effects ?? []).join(" "));
}
