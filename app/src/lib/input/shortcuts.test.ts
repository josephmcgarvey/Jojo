import { describe, expect, it } from "vitest";
import { MAC_SHORTCUT_LABELS, resolveShortcut, SHORTCUT_LABELS, WINDOWS_SHORTCUT_LABELS, type AppAction, type KeyInput } from "./shortcuts";

function key(code: string, mods: Partial<KeyInput> = {}, keyValue = code): KeyInput {
  return { key: keyValue, code, ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...mods };
}

const copyOn = { copyOnSelect: true, hasSelection: false };

describe("resolveShortcut", () => {
  it("maps every Plan 3 shortcut from spec 9.6", () => {
    const cases: [KeyInput, AppAction][] = [
      [key("KeyT", { ctrlKey: true, shiftKey: true }, "T"), "new-claude-tab"],
      [key("KeyN", { ctrlKey: true, shiftKey: true }, "N"), "new-shell-tab"],
      [key("Equal", { altKey: true, shiftKey: true }, "+"), "split-right"],
      [key("Minus", { altKey: true, shiftKey: true }, "_"), "split-down"],
      [key("KeyZ", { altKey: true, shiftKey: true }, "Z"), "zoom"],
      [key("ArrowLeft", { altKey: true }), "focus-left"],
      [key("ArrowRight", { altKey: true }), "focus-right"],
      [key("ArrowUp", { altKey: true }), "focus-up"],
      [key("ArrowDown", { altKey: true }), "focus-down"],
      [key("Tab", { ctrlKey: true }), "next-tab"],
      [key("Tab", { ctrlKey: true, shiftKey: true }), "previous-tab"],
      [key("KeyW", { ctrlKey: true, shiftKey: true }, "W"), "close-pane"],
      [key("KeyC", { ctrlKey: true, shiftKey: true }, "C"), "copy"],
      [key("KeyV", { ctrlKey: true, shiftKey: true }, "V"), "paste"],
    ];
    for (const [input, action] of cases) expect(resolveShortcut(input, copyOn), `${input.code}`).toBe(action);
  });

  it("always sends Ctrl+C to the terminal when copyOnSelect is on", () => {
    const ctrlC = key("KeyC", { ctrlKey: true }, "c");
    expect(resolveShortcut(ctrlC, { copyOnSelect: true, hasSelection: true })).toBeNull();
    expect(resolveShortcut(ctrlC, { copyOnSelect: true, hasSelection: false })).toBeNull();
  });

  it("copies with Ctrl+C when copyOnSelect is off and text is selected, else sends it", () => {
    const ctrlC = key("KeyC", { ctrlKey: true }, "c");
    expect(resolveShortcut(ctrlC, { copyOnSelect: false, hasSelection: true })).toBe("copy");
    expect(resolveShortcut(ctrlC, { copyOnSelect: false, hasSelection: false })).toBeNull();
  });

  it("leaves plain Ctrl combinations, AltGr and the Windows key to the terminal", () => {
    expect(resolveShortcut(key("KeyT", { ctrlKey: true }, "t"), copyOn)).toBeNull();
    expect(resolveShortcut(key("KeyK", { ctrlKey: true }, "k"), copyOn)).toBeNull();
    expect(resolveShortcut(key("KeyV", { ctrlKey: true }, "v"), copyOn)).toBeNull();
    expect(resolveShortcut(key("ArrowLeft", { ctrlKey: true, altKey: true }), copyOn)).toBeNull();
    expect(resolveShortcut(key("KeyT", { ctrlKey: true, shiftKey: true, metaKey: true }, "T"), copyOn)).toBeNull();
    expect(resolveShortcut(key("KeyP", { ctrlKey: true, shiftKey: true }, "P"), copyOn)).toBe("switcher");
    expect(resolveShortcut(key("KeyJ", { ctrlKey: true, shiftKey: true }, "J"), copyOn)).toBe("jump");
    expect(resolveShortcut(key("KeyA"), copyOn)).toBeNull();
  });

  it("uses Command on macOS and leaves Ctrl and Option to the terminal", () => {
    const mac = (input: KeyInput) => resolveShortcut(input, copyOn, true);
    expect(mac(key("KeyT", { metaKey: true }, "t"))).toBe("new-claude-tab");
    expect(mac(key("KeyN", { metaKey: true }, "n"))).toBe("new-shell-tab");
    expect(mac(key("KeyW", { metaKey: true }, "w"))).toBe("close-pane");
    expect(mac(key("KeyC", { metaKey: true }, "c"))).toBe("copy");
    expect(mac(key("KeyV", { metaKey: true }, "v"))).toBe("paste");
    expect(mac(key("KeyP", { metaKey: true }, "p"))).toBe("switcher");
    expect(mac(key("KeyD", { metaKey: true }, "d"))).toBe("split-right");
    expect(mac(key("KeyD", { metaKey: true, shiftKey: true }, "D"))).toBe("split-down");
    expect(mac(key("Enter", { metaKey: true, shiftKey: true }, "Enter"))).toBe("zoom");
    expect(mac(key("BracketRight", { metaKey: true, shiftKey: true }, "}"))).toBe("next-tab");
    expect(mac(key("BracketLeft", { metaKey: true, shiftKey: true }, "{"))).toBe("previous-tab");
    expect(mac(key("ArrowLeft", { metaKey: true, altKey: true }))).toBe("focus-left");
    expect(mac(key("Tab", { ctrlKey: true }))).toBe("next-tab");
    // The terminal's keys: Option+Arrow (word movement), Ctrl+C, Ctrl+Shift+T, ⌘Q (the app menu).
    expect(mac(key("ArrowLeft", { altKey: true }))).toBeNull();
    expect(mac(key("KeyC", { ctrlKey: true }, "c"))).toBeNull();
    expect(mac(key("KeyT", { ctrlKey: true, shiftKey: true }, "T"))).toBeNull();
    expect(mac(key("Equal", { altKey: true, shiftKey: true }, "±"))).toBeNull();
    expect(mac(key("KeyQ", { metaKey: true }, "q"))).toBeNull();
    for (const action of Object.keys(WINDOWS_SHORTCUT_LABELS) as AppAction[]) expect(MAC_SHORTCUT_LABELS[action], action).toBeTruthy();
  });

  it("has a menu label for every action", () => {
    expect(WINDOWS_SHORTCUT_LABELS["split-right"]).toBe("Alt+Shift+=");
    expect(WINDOWS_SHORTCUT_LABELS.zoom).toBe("Alt+Shift+Z");
    expect(WINDOWS_SHORTCUT_LABELS["close-pane"]).toBe("Ctrl+Shift+W");
    expect(SHORTCUT_LABELS).toBe(WINDOWS_SHORTCUT_LABELS);
  });
});
