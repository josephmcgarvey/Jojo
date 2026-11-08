import { describe, expect, it } from "vitest";
import { resolveShortcut, SHORTCUT_LABELS, type AppAction, type KeyInput } from "./shortcuts";

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

  it("has a menu label for every action", () => {
    expect(SHORTCUT_LABELS["split-right"]).toBe("Alt+Shift+=");
    expect(SHORTCUT_LABELS.zoom).toBe("Alt+Shift+Z");
    expect(SHORTCUT_LABELS["close-pane"]).toBe("Ctrl+Shift+W");
  });
});
