// A theme is one JSON file: `tokens` become --jojo-* custom properties on
// :root, `terminal` is the xterm.js palette (spec section 9.9). `accent` is
// the base color the theme is drawn around (see palette.ts).
import type { ITheme } from "@xterm/xterm";
import daylight from "../themes/daylight.json";
import neonGlass from "../themes/neon-glass.json";
import { uiFontStack } from "./fonts";
import { recolorTheme } from "./palette";

export interface Theme {
  name: string;
  mode: "dark" | "light";
  accent: string;
  tokens: Record<string, string>;
  terminal: ITheme;
}

/** The themes in Settings ▸ Theme, by the id stored in settings.json. */
export const THEMES: Record<string, Theme> = {
  "neon-glass": neonGlass as Theme,
  daylight: daylight as Theme,
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
}

/** Writes the tokens as --jojo-* properties and sets color-scheme, which colors scrollbars and native controls. */
export function applyTheme(theme: Theme, root: StyleTarget = document.documentElement): void {
  for (const [name, value] of Object.entries(theme.tokens)) {
    root.style.setProperty(`--jojo-${name}`, value);
  }
  root.style.setProperty("color-scheme", theme.mode);
}
