// Replays each snapshot written by crates/jojo-grid/tests/recordings.rs into
// @xterm/headless (the emulator the app uses) and compares the visible text
// with alacritty_terminal's (spec section 11, item 1). Run the Rust test
// first: it writes tests/recordings/out.
import { Terminal } from "@xterm/headless";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const outDir = join(import.meta.dirname, "..", "recordings", "out");
const names = readdirSync(outDir)
  .filter((f) => f.endsWith(".snap"))
  .map((f) => f.slice(0, -".snap".length));

function visibleText(term: Terminal, rows: number): string {
  const buffer = term.buffer.active;
  const lines: string[] = [];
  for (let y = 0; y < rows; y++) {
    lines.push(buffer.getLine(buffer.baseY + y)?.translateToString(true) ?? "");
  }
  while (lines.length > 0 && lines[lines.length - 1] === "") lines.pop();
  return lines.join("\n");
}

describe("snapshots replay identically in xterm.js", () => {
  it("found recordings", () => expect(names.length).toBeGreaterThanOrEqual(3));

  for (const name of names) {
    it(name, async () => {
      const [cols, rows] = readFileSync(join(outDir, `${name}.size`), "utf8").split(" ").map(Number);
      const term = new Terminal({ cols, rows, scrollback: 10_000, allowProposedApi: true });
      term.loadAddon(new Unicode11Addon());
      term.unicode.activeVersion = "11";
      const snapshot = readFileSync(join(outDir, `${name}.snap`));
      await new Promise<void>((resolve) => term.write(new Uint8Array(snapshot), resolve));
      const expected = readFileSync(join(outDir, `${name}.txt`), "utf8");
      expect(visibleText(term, rows).split("\n").map((l) => l.trimEnd()).join("\n")).toBe(expected);
    });
  }
});
