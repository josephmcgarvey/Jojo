import { Terminal } from "@xterm/headless";
import { describe, expect, it } from "vitest";
import { handleClipboardWrites, osc52Text } from "./osc52";

const b64 = (text: string) => btoa(String.fromCharCode(...new TextEncoder().encode(text)));

describe("osc52Text", () => {
  it("decodes the base64 UTF-8 text after the selection parameter", () => {
    expect(osc52Text(`c;${b64("hello world")}`)).toBe("hello world");
    expect(osc52Text(`;${b64("café → ok")}`)).toBe("café → ok");
    expect(osc52Text(`pc;${b64("two\nlines")}`)).toBe("two\nlines");
  });

  it("returns an empty string for an empty payload", () => {
    expect(osc52Text("c;")).toBe("");
  });

  it("refuses clipboard queries and malformed payloads", () => {
    expect(osc52Text("c;?")).toBeNull();
    expect(osc52Text("no separator")).toBeNull();
    expect(osc52Text("c;not base64!")).toBeNull();
    expect(osc52Text("c;/w==")).toBeNull(); // 0xFF is not UTF-8
  });
});

describe("handleClipboardWrites", () => {
  async function copiesFrom(output: string): Promise<{ copied: string[]; replies: string }> {
    const term = new Terminal({ cols: 80, rows: 24, allowProposedApi: true });
    const copied: string[] = [];
    handleClipboardWrites(term, (text) => copied.push(text));
    let replies = "";
    term.onData((d) => (replies += d));
    await new Promise<void>((resolve) => term.write(output, resolve));
    term.dispose();
    return { copied, replies };
  }

  it("copies the text of an OSC 52 write ended by BEL or ST", async () => {
    const { copied } = await copiesFrom(`\x1b]52;c;${b64("first")}\x07\x1b]52;c;${b64("second")}\x1b\\`);
    expect(copied).toEqual(["first", "second"]);
  });

  it("answers no clipboard query", async () => {
    expect(await copiesFrom("\x1b]52;c;?\x07")).toEqual({ copied: [], replies: "" });
  });
});
