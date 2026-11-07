import { describe, expect, it } from "vitest";
import { firstEnabled, moveHighlight, placeMenu, placeSubmenu } from "./navigation";
import { isEnabled, SEPARATOR, type MenuEntry } from "./types";

const run = () => {};
const entries: MenuEntry[] = [
  { kind: "action", label: "Copy", disabledReason: "No text selected", run },
  { kind: "action", label: "Paste", run },
  SEPARATOR,
  { kind: "submenu", label: "Swap with", items: [{ kind: "action", label: "x", disabledReason: "no", run }] },
  { kind: "action", label: "Close pane", run },
];

describe("isEnabled", () => {
  it("disables separators, entries with a reason, and submenus whose items are all disabled", () => {
    expect(entries.map(isEnabled)).toEqual([false, true, false, false, true]);
  });
});

describe("moveHighlight", () => {
  it("skips separators and disabled entries and wraps in both directions", () => {
    expect(moveHighlight(entries, 1, 1)).toBe(4);
    expect(moveHighlight(entries, 4, 1)).toBe(1);
    expect(moveHighlight(entries, 1, -1)).toBe(4);
  });

  it("starts at the first or last enabled entry when nothing is highlighted", () => {
    expect(moveHighlight(entries, -1, 1)).toBe(1);
    expect(moveHighlight(entries, -1, -1)).toBe(4);
    expect(firstEnabled(entries)).toBe(1);
  });

  it("returns -1 when no entry is enabled", () => {
    expect(moveHighlight([SEPARATOR], -1, 1)).toBe(-1);
    expect(moveHighlight([], 0, 1)).toBe(-1);
  });
});

describe("placement", () => {
  const viewport = { w: 800, h: 600 };

  it("opens a menu at the pointer when it fits", () => {
    expect(placeMenu({ x: 10, y: 10 }, { w: 100, h: 50 }, viewport)).toEqual({ x: 10, y: 10 });
  });

  it("flips a menu left of the pointer and moves it up to stay inside the window", () => {
    expect(placeMenu({ x: 750, y: 580 }, { w: 100, h: 50 }, viewport)).toEqual({ x: 650, y: 546 });
  });

  it("opens a submenu to the right of its row, or to the left when there is no room", () => {
    expect(placeSubmenu({ x: 100, y: 100, w: 150, h: 24 }, { w: 120, h: 80 }, viewport)).toEqual({ x: 250, y: 100 });
    expect(placeSubmenu({ x: 600, y: 560, w: 150, h: 24 }, { w: 120, h: 80 }, viewport)).toEqual({ x: 480, y: 516 });
  });
});
