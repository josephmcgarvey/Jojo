import { Terminal } from "@xterm/headless";
import { describe, expect, it } from "vitest";
import { suppressQueryReplies } from "./queries";

const QUERIES = "\x1b[6n\x1b[5n\x1b[c\x1b[>c\x1b[4$p\x1b[?25$p";

async function repliesTo(suppress: boolean): Promise<string> {
  const term = new Terminal({ cols: 80, rows: 24, allowProposedApi: true });
  if (suppress) suppressQueryReplies(term);
  let replies = "";
  term.onData((d) => (replies += d));
  await new Promise<void>((resolve) => term.write(QUERIES, resolve));
  term.dispose();
  return replies;
}

describe("suppressQueryReplies", () => {
  it("xterm.js answers these queries by default (control)", async () => {
    expect(await repliesTo(false)).not.toBe("");
  });

  it("stops xterm.js from answering DSR, DA1, DA2 and DECRQM", async () => {
    expect(await repliesTo(true)).toBe("");
  });
});
