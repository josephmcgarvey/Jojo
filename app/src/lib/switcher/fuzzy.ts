// Fuzzy matching for the switcher (spec 9.6, Ctrl+Shift+P).

/** Characters after which a letter starts a word: "f" in "fix-login" or "fix login". */
const WORD_BREAK = /[\s\-_/\\.:·]/;

/**
 * Scores `text` against `query`, or returns null when the query's
 * characters do not all appear in order. Case is ignored. Each query
 * character is matched at its next occurrence that follows the previous
 * match directly, else at its next occurrence that starts a word, else at
 * its next occurrence. Higher is better: each matched character earns 1,
 * plus 4 when it follows the previous match directly, plus 3 when it starts
 * a word; matches nearer the start earn a little more. An empty query
 * matches everything with score 0.
 */
export function fuzzyScore(query: string, text: string): number | null {
  const q = query.trim().toLowerCase();
  if (q === "") return 0;
  const t = text.toLowerCase();
  let score = 0;
  let from = 0;
  let previous = -2;
  for (const ch of q) {
    if (ch === " ") continue;
    const at = nextMatch(t, ch, from, previous);
    if (at < 0) return null;
    score += 1;
    if (at === previous + 1) score += 4;
    else if (at === 0 || WORD_BREAK.test(t[at - 1])) score += 3;
    score += Math.max(0, 1 - at / 100);
    previous = at;
    from = at + 1;
  }
  return score;
}

function nextMatch(text: string, ch: string, from: number, previous: number): number {
  if (previous >= 0 && text[previous + 1] === ch) return previous + 1;
  for (let i = text.indexOf(ch, from); i >= 0; i = text.indexOf(ch, i + 1)) {
    if (i === 0 || WORD_BREAK.test(text[i - 1])) return i;
  }
  return text.indexOf(ch, from);
}

/** The items whose text matches `query`, best first; ties keep their order. */
export function fuzzyFilter<T>(items: T[], query: string, textOf: (item: T) => string): T[] {
  return items
    .map((item, index) => ({ item, index, score: fuzzyScore(query, textOf(item)) }))
    .filter((entry): entry is { item: T; index: number; score: number } => entry.score !== null)
    .sort((a, b) => b.score - a.score || a.index - b.index)
    .map((entry) => entry.item);
}
