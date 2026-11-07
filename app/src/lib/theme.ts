// A theme is one JSON file: `tokens` become --jojo-* custom properties on
// :root, `terminal` is the xterm.js palette (spec section 9.9).
import type { ITheme } from "@xterm/xterm";

export interface Theme {
  name: string;
  tokens: Record<string, string>;
  terminal: ITheme;
}

interface StyleTarget {
  style: { setProperty(name: string, value: string): void };
}

export function applyTheme(theme: Theme, root: StyleTarget = document.documentElement): void {
  for (const [name, value] of Object.entries(theme.tokens)) {
    root.style.setProperty(`--jojo-${name}`, value);
  }
}
