// OSC 52, the escape sequence a program sends to set the clipboard of the
// terminal it is shown in: `ESC ] 52 ; <selection> ; <base64 text> BEL`.
//
// A program on a server cannot reach this computer's clipboard any other
// way. Claude Code copies its own mouse selection through the system
// clipboard tool when it runs on this computer (PowerShell, pbcopy), but
// over SSH it sends only OSC 52; xterm.js ignores the sequence unless a
// handler is registered, so without this module a copy in a remote Claude
// Code session printed "Copied N characters" and left the clipboard as it was.
//
// Only writes are honored. A `?` in place of the text asks the terminal to
// send the clipboard back to the program, which would let any program on a
// server read what was copied on this computer, so a query is consumed and
// nothing is replied.
interface OscParser {
  registerOscHandler(ident: number, callback: (data: string) => boolean): { dispose(): void };
}

/**
 * The text an OSC 52 payload (`<selection>;<base64>`) asks to copy, or null
 * when it is a query (`?`), malformed or not valid base64 UTF-8. An empty
 * text is returned as "": xterm and kitty clear the clipboard for it.
 */
export function osc52Text(data: string): string | null {
  const separator = data.indexOf(";");
  if (separator < 0) return null;
  const encoded = data.slice(separator + 1).trim();
  if (encoded === "?") return null;
  if (!/^[A-Za-z0-9+/]*={0,2}$/.test(encoded)) return null;
  try {
    const bytes = Uint8Array.from(atob(encoded), (c) => c.charCodeAt(0));
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    return null;
  }
}

/** Copies the text of every OSC 52 write the terminal receives with `copy`. */
export function handleClipboardWrites(term: { parser: OscParser }, copy: (text: string) => void): { dispose(): void } {
  return term.parser.registerOscHandler(52, (data) => {
    const text = osc52Text(data);
    if (text !== null) copy(text);
    return true;
  });
}
