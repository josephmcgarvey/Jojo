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
  const chars = [...q].filter((c) => c !== " ");
  for (let n = 0; n < chars.length; n++) {
    const ch = chars[n];
    const remaining = chars.slice(n + 1).join("");
    const at = nextMatch(t, ch, from, previous, remaining);
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

/**
 * Where `ch` is matched. A preferred position (right after the previous
 * match, or a word start) is taken only when `remaining` (the rest of the
 * query) can still be matched after it; otherwise the earliest occurrence,
 * so preferring a position never turns a match into a miss.
 */
function nextMatch(text: string, ch: string, from: number, previous: number, remaining: string): number {
  const fits = (at: number) => isSubsequence(remaining, text, at + 1);
  if (previous >= 0 && text[previous + 1] === ch && fits(previous + 1)) return previous + 1;
  for (let i = text.indexOf(ch, from); i >= 0; i = text.indexOf(ch, i + 1)) {
    if ((i === 0 || WORD_BREAK.test(text[i - 1])) && fits(i)) return i;
  }
  return text.indexOf(ch, from);
}

/** True when the characters of `query` appear in order in `text` from `from`. */
function isSubsequence(query: string, text: string, from: number): boolean {
  let at = from;
  for (const ch of query) {
    at = text.indexOf(ch, at);
    if (at < 0) return false;
    at += 1;
  }
  return true;
}

/** The items whose text matches `query`, best first; ties keep their order. */
export function fuzzyFilter<T>(items: T[], query: string, textOf: (item: T) => string): T[] {
  return items
    .map((item, index) => ({ item, index, score: fuzzyScore(query, textOf(item)) }))
    .filter((entry): entry is { item: T; index: number; score: number } => entry.score !== null)
    .sort((a, b) => b.score - a.score || a.index - b.index)
    .map((entry) => entry.item);
}
