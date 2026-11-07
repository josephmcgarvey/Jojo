import { describe, expect, it, vi } from "vitest";
import neonGlass from "../themes/neon-glass.json";
import { applyTheme, type Theme } from "./theme";

describe("theme", () => {
  it("writes every token as a --jojo- custom property", () => {
    const setProperty = vi.fn();
    applyTheme(neonGlass as Theme, { style: { setProperty } });
    expect(setProperty).toHaveBeenCalledWith("--jojo-bg", "#070a16");
    expect(setProperty).toHaveBeenCalledTimes(Object.keys(neonGlass.tokens).length);
  });

  it("defines the tokens and terminal colors the UI uses", () => {
    const required = ["bg", "bg-glow", "side", "bar", "pane", "line", "sel", "fg", "fg2", "muted", "accent", "working", "needs-you", "done", "error", "idle", "font-ui", "font-mono", "brand-gradient", "radius", "radius-sm", "danger"];
    for (const name of required) expect(neonGlass.tokens, name).toHaveProperty(name);
    for (const key of ["background", "foreground", "cursor", "selectionBackground", "black", "brightWhite"]) {
      expect(neonGlass.terminal, key).toHaveProperty(key);
    }
  });
});
