// The fonts offered in Settings. "bundled" fonts ship with Jojo (@fontsource
// packages imported in main.ts); "windows" fonts come with Windows 11
// (Cascadia Code and Cascadia Mono since Windows 11, Consolas and Segoe UI
// since Vista); "macos" fonts come with macOS (Menlo since 10.6, Monaco, and
// the system interface font, San Francisco, named by the CSS keyword
// system-ui). Each list holds the bundled fonts and this platform's own.
// Settings stores only the family name; the stacks below add fallbacks for
// a family that is not installed.
import { isMac } from "./platform";

export interface FontChoice {
  family: string;
  source: "bundled" | "windows" | "macos";
  /** Menu text when it differs from the family name. */
  label?: string;
}

const ALL_TERMINAL_FONTS: FontChoice[] = [
  { family: "Geist Mono", source: "bundled" },
  { family: "JetBrains Mono", source: "bundled" },
  { family: "Fira Code", source: "bundled" },
  { family: "IBM Plex Mono", source: "bundled" },
  { family: "Cascadia Code", source: "windows" },
  { family: "Cascadia Mono", source: "windows" },
  { family: "Consolas", source: "windows" },
  { family: "Menlo", source: "macos" },
  { family: "Monaco", source: "macos" },
];

const ALL_UI_FONTS: FontChoice[] = [
  { family: "Geist Sans", source: "bundled" },
  { family: "Inter", source: "bundled" },
  { family: "Segoe UI", source: "windows" },
  { family: "system-ui", source: "macos", label: "San Francisco (system)" },
];

const PLATFORM_SOURCE = isMac ? "macos" : "windows";
export const TERMINAL_FONTS = ALL_TERMINAL_FONTS.filter((f) => f.source === "bundled" || f.source === PLATFORM_SOURCE);
export const UI_FONTS = ALL_UI_FONTS.filter((f) => f.source === "bundled" || f.source === PLATFORM_SOURCE);

/** Menu text for a font: its label or family, with the platform it comes with. */
export function fontLabel(font: FontChoice): string {
  const name = font.label ?? font.family;
  if (font.source === "windows") return `${name} (Windows)`;
  if (font.source === "macos" && !font.label) return `${name} (macOS)`;
  return name;
}

/** CSS generic family keywords, which must stay unquoted to keep their meaning. */
const GENERIC_FAMILIES = new Set(["system-ui", "-apple-system", "monospace", "sans-serif", "serif", "ui-monospace", "ui-sans-serif"]);

/** A family name as a CSS string; double quotes and backslashes are dropped so the value stays one name. */
function quoted(family: string): string {
  const name = family.replace(/["\\]/g, "").trim();
  return GENERIC_FAMILIES.has(name) ? name : `"${name}"`;
}

/** The xterm.js fontFamily for a terminal font: the family, then Windows and macOS monospace fallbacks. */
export function terminalFontStack(family: string): string {
  return `${quoted(family)}, "Cascadia Mono", Consolas, Menlo, monospace`;
}

/** The --jojo-font-ui value for an interface font. */
export function uiFontStack(family: string): string {
  return `${quoted(family)}, "Segoe UI", system-ui, sans-serif`;
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
