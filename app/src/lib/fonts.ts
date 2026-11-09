// The fonts offered in Settings. "bundled" fonts ship with Jojo (@fontsource
// packages imported in main.ts); "windows" fonts come with Windows 11
// (Cascadia Code and Cascadia Mono since Windows 11, Consolas and Segoe UI
// since Vista). Settings stores only the family name; the stacks below add
// fallbacks for a family that is not installed.

export interface FontChoice {
  family: string;
  source: "bundled" | "windows";
}

export const TERMINAL_FONTS: FontChoice[] = [
  { family: "Geist Mono", source: "bundled" },
  { family: "JetBrains Mono", source: "bundled" },
  { family: "Fira Code", source: "bundled" },
  { family: "IBM Plex Mono", source: "bundled" },
  { family: "Cascadia Code", source: "windows" },
  { family: "Cascadia Mono", source: "windows" },
  { family: "Consolas", source: "windows" },
];

export const UI_FONTS: FontChoice[] = [
  { family: "Geist Sans", source: "bundled" },
  { family: "Inter", source: "bundled" },
  { family: "Segoe UI", source: "windows" },
];

/** A family name as a CSS string; double quotes and backslashes are dropped so the value stays one name. */
function quoted(family: string): string {
  return `"${family.replace(/["\\]/g, "").trim()}"`;
}

/** The xterm.js fontFamily for a terminal font: the family, then Windows monospace fallbacks. */
export function terminalFontStack(family: string): string {
  return `${quoted(family)}, "Cascadia Mono", Consolas, monospace`;
}

/** The --jojo-font-ui value for an interface font. */
export function uiFontStack(family: string): string {
  return `${quoted(family)}, "Segoe UI", sans-serif`;
}

/**
 * Loads the regular and bold faces of a terminal font. xterm.js measures its
 * character cell when a terminal opens or its font changes; measuring before
 * a bundled face has loaded (they use font-display: swap and load on first
 * use) gives the fallback font's cell size. A failed load is ignored, and
 * the terminal uses the next font in the stack.
 */
export async function loadTerminalFont(family: string): Promise<void> {
  const stack = terminalFontStack(family);
  try {
    await Promise.all([document.fonts.load(`400 13px ${stack}`), document.fonts.load(`700 13px ${stack}`)]);
  } catch {
    // The fallback font is used.
  }
}
