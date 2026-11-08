// Spec 9.4, mouse reporting rule. When the program in a pane has enabled
// mouse reporting (vim, htop, any TUI in mouse mode), a plain right-click
// goes to the program and Shift+right-click opens Jojo's menu; when
// reporting is off, a plain right-click opens Jojo's menu.
//
// Spec 9.5 rule 4 (Shift+drag forces a selection while mouse reporting is
// on) needs no code here: on Windows, xterm.js's
// SelectionService.shouldForceSelection returns event.shiftKey, and its
// mouse service then does not report the press to the program.

/** The values of xterm.js 6 `term.modes.mouseTrackingMode`. */
export type MouseTrackingMode = "none" | "x10" | "vt200" | "drag" | "any";

export function contextMenuTarget(shiftKey: boolean, mode: MouseTrackingMode): "jojo" | "program" {
  return shiftKey || mode === "none" ? "jojo" : "program";
}
