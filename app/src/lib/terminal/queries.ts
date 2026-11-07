// jojod answers terminal queries itself (Plan 2a carry-over, decision 3
// of Plan 1). If xterm.js also answered, programs would receive two
// replies. Returning true from a CSI handler marks the sequence handled, so
// xterm.js sends nothing.
interface CsiParser {
  registerCsiHandler(
    id: { prefix?: string; intermediates?: string; final: string },
    callback: () => boolean,
  ): { dispose(): void };
}

const QUERIES = [
  { final: "n" }, // DSR, including cursor position report (CSI 6 n)
  { final: "c" }, // DA1
  { prefix: ">", final: "c" }, // DA2
  { intermediates: "$", final: "p" }, // DECRQM, ANSI modes
  { prefix: "?", intermediates: "$", final: "p" }, // DECRQM, DEC private modes
];

export function suppressQueryReplies(term: { parser: CsiParser }): { dispose(): void }[] {
  return QUERIES.map((id) => term.parser.registerCsiHandler(id, () => true));
}
