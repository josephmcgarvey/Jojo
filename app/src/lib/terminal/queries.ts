// jojod answers terminal queries itself (Plan 2a carry-over, decision 3
// of Plan 1). If xterm.js also answered, programs would receive two
// replies. Returning true from a CSI handler marks the sequence handled, so
// xterm.js sends nothing.
//
// That includes the color queries OSC 4 (palette), 10 (foreground), 11
// (background) and 12 (cursor): xterm.js's answer arrived after jojod's
// answer to the cursor position request that programs send after a color
// query, and leaked into the shell as typed text (jojo-grid's
// ReplyCollector). Only queries (a `?` parameter) are handled here; an OSC
// that sets a color still reaches xterm.js, which shows the new color.
interface CsiParser {
  registerCsiHandler(
    id: { prefix?: string; intermediates?: string; final: string },
    callback: () => boolean,
  ): { dispose(): void };
  registerOscHandler(ident: number, callback: (data: string) => boolean): { dispose(): void };
}

const COLOR_QUERIES = [4, 10, 11, 12];

const QUERIES = [
  { final: "n" }, // DSR, including cursor position report (CSI 6 n)
  { prefix: "?", final: "n" }, // DECXCPR, extended cursor position report (CSI ? 6 n)
  { final: "c" }, // DA1
  { prefix: ">", final: "c" }, // DA2
  { intermediates: "$", final: "p" }, // DECRQM, ANSI modes
  { prefix: "?", intermediates: "$", final: "p" }, // DECRQM, DEC private modes
];

export function suppressQueryReplies(term: { parser: CsiParser }): { dispose(): void }[] {
  return [
    ...QUERIES.map((id) => term.parser.registerCsiHandler(id, () => true)),
    ...COLOR_QUERIES.map((ident) => term.parser.registerOscHandler(ident, (data) => data.split(";").includes("?"))),
  ];
}
