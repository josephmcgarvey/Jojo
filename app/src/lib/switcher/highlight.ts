// Splits a line of output around every occurrence of the searched text, so
// the switcher can mark the matches.

export interface TextPart {
  text: string;
  match: boolean;
}

/** `text` as parts, the ones equal to `query` (ignoring case) marked; one unmarked part when there is no match. */
export function highlightParts(text: string, query: string): TextPart[] {
  const needle = query.trim().toLowerCase();
  if (!needle) return [{ text, match: false }];
  const lower = text.toLowerCase();
  const parts: TextPart[] = [];
  let from = 0;
  for (let at = lower.indexOf(needle); at >= 0; at = lower.indexOf(needle, at + needle.length)) {
    if (at > from) parts.push({ text: text.slice(from, at), match: false });
    parts.push({ text: text.slice(at, at + needle.length), match: true });
    from = at + needle.length;
  }
  if (from < text.length || parts.length === 0) parts.push({ text: text.slice(from), match: false });
  return parts;
}
