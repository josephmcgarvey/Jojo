// Application keyboard shortcuts (spec 9.6) and the Ctrl+C rules (spec 9.5).
// Application shortcuts use Ctrl+Shift or Alt+Shift, plus Ctrl+Tab,
// Ctrl+Shift+Tab and Alt+Arrow. Plain Ctrl combinations reach the terminal,
// because Claude Code and shells bind them (Claude Code binds Ctrl+T;
// readline binds Ctrl+K), with one exception: Ctrl+C copies when
// copyOnSelect is off and text is selected. Letter and punctuation keys are
// matched by KeyboardEvent.code (the physical key), because with Shift held
// KeyboardEvent.key is "T" or "+" rather than "t" or "=".
//
// On macOS the application shortcuts use Command instead, as other Mac
// terminals do (resolveMacShortcut), because Ctrl and Option belong to the
// terminal there: Option+Arrow moves by word in zsh, Option+Shift types
// characters such as ± on many layouts, and Ctrl+letter is a control code.
import { isMac } from "../platform";

export type AppAction =
  | "new-claude-tab"
  | "new-shell-tab"
  | "split-right"
  | "split-down"
  | "zoom"
  | "focus-left"
  | "focus-right"
  | "focus-up"
  | "focus-down"
  | "next-tab"
  | "previous-tab"
  | "close-pane"
  | "copy"
  | "paste"
  | "switcher"
  | "jump"
  | "toggle-sidebar";

export interface KeyInput {
  key: string;
  code: string;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  metaKey: boolean;
}

export interface ShortcutContext {
  copyOnSelect: boolean;
  /** Whether the focused pane has selected text. */
  hasSelection: boolean;
}

/** Windows shortcut text shown next to menu items (spec 9.4). */
export const WINDOWS_SHORTCUT_LABELS: Record<AppAction, string> = {
  "new-claude-tab": "Ctrl+Shift+T",
  "new-shell-tab": "Ctrl+Shift+N",
  "split-right": "Alt+Shift+=",
  "split-down": "Alt+Shift+-",
  zoom: "Alt+Shift+Z",
  "focus-left": "Alt+Left",
  "focus-right": "Alt+Right",
  "focus-up": "Alt+Up",
  "focus-down": "Alt+Down",
  "next-tab": "Ctrl+Tab",
  "previous-tab": "Ctrl+Shift+Tab",
  "close-pane": "Ctrl+Shift+W",
  copy: "Ctrl+Shift+C",
  paste: "Ctrl+Shift+V",
  switcher: "Ctrl+Shift+P",
  jump: "Ctrl+Shift+J",
  "toggle-sidebar": "Ctrl+Shift+B",
};

/** macOS shortcut text, with the menu glyphs macOS uses (⌘ Command, ⌥ Option, ⇧ Shift, ⌃ Control). */
export const MAC_SHORTCUT_LABELS: Record<AppAction, string> = {
  "new-claude-tab": "⌘T",
  "new-shell-tab": "⌘N",
  "split-right": "⌘D",
  "split-down": "⇧⌘D",
  zoom: "⇧⌘↩",
  "focus-left": "⌥⌘←",
  "focus-right": "⌥⌘→",
  "focus-up": "⌥⌘↑",
  "focus-down": "⌥⌘↓",
  "next-tab": "⇧⌘]",
  "previous-tab": "⇧⌘[",
  "close-pane": "⌘W",
  copy: "⌘C",
  paste: "⌘V",
  switcher: "⌘P",
  jump: "⌘J",
  "toggle-sidebar": "⌘B",
};

/** Shortcut text for this platform. */
export const SHORTCUT_LABELS = isMac ? MAC_SHORTCUT_LABELS : WINDOWS_SHORTCUT_LABELS;

const CMD: Record<string, AppAction> = {
  KeyT: "new-claude-tab",
  KeyN: "new-shell-tab",
  KeyW: "close-pane",
  KeyC: "copy",
  KeyV: "paste",
  KeyP: "switcher",
  KeyJ: "jump",
  KeyD: "split-right",
  KeyB: "toggle-sidebar",
};

const CMD_SHIFT: Record<string, AppAction> = {
  KeyD: "split-down",
  Enter: "zoom",
  BracketLeft: "previous-tab",
  BracketRight: "next-tab",
};

const CTRL_SHIFT: Record<string, AppAction> = {
  KeyT: "new-claude-tab",
  KeyN: "new-shell-tab",
  KeyW: "close-pane",
  KeyC: "copy",
  KeyV: "paste",
  Tab: "previous-tab",
  KeyP: "switcher",
  KeyJ: "jump",
  KeyB: "toggle-sidebar",
};

const ALT_SHIFT: Record<string, AppAction> = {
  Equal: "split-right",
  Minus: "split-down",
  KeyZ: "zoom",
};

const ALT_ARROWS: Record<string, AppAction> = {
  ArrowLeft: "focus-left",
  ArrowRight: "focus-right",
  ArrowUp: "focus-up",
  ArrowDown: "focus-down",
};

/**
 * The application action for a key event, or null when the key belongs to
 * the terminal. On Windows, Ctrl+Alt is AltGr on many keyboard layouts and
 * is never an application shortcut; neither is anything with the Windows
 * key. `mac` selects the macOS bindings (resolveMacShortcut).
 */
export function resolveShortcut(event: KeyInput, context: ShortcutContext, mac: boolean = isMac): AppAction | null {
  if (mac) return resolveMacShortcut(event);
  const { ctrlKey: ctrl, shiftKey: shift, altKey: alt, metaKey: meta } = event;
  if (meta) return null;
  if (ctrl && shift && !alt) return CTRL_SHIFT[event.code] ?? null;
  if (alt && shift && !ctrl) return ALT_SHIFT[event.code] ?? null;
  if (alt && !shift && !ctrl) return ALT_ARROWS[event.key] ?? null;
  if (ctrl && !shift && !alt) {
    if (event.code === "Tab") return "next-tab";
    if (event.code === "KeyC" && !context.copyOnSelect && context.hasSelection) return "copy";
  }
  return null;
}

/**
 * macOS bindings: Command with a letter (⌘T, ⌘N, ⌘W, ⌘C, ⌘V, ⌘P, ⌘J, ⌘D),
 * Shift+Command (⇧⌘D, ⇧⌘↩, ⇧⌘[ and ⇧⌘]), Option+Command+Arrow, and
 * Ctrl+Tab / Ctrl+Shift+Tab. Every other Command key (⌘Q, ⌘H, ⌘M) is left
 * to the application menu; every Ctrl and Option key reaches the terminal.
 */
function resolveMacShortcut(event: KeyInput): AppAction | null {
  const { ctrlKey: ctrl, shiftKey: shift, altKey: alt, metaKey: meta } = event;
  if (meta && !ctrl && !alt) return (shift ? CMD_SHIFT : CMD)[event.code] ?? null;
  if (meta && alt && !ctrl && !shift) return ALT_ARROWS[event.key] ?? null;
  if (ctrl && !meta && !alt && event.code === "Tab") return shift ? "previous-tab" : "next-tab";
  return null;
}
