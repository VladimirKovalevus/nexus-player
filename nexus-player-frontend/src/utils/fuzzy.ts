export function fuzzyScore(needle: string, haystack: string): number | null {
  const n = needle.toLowerCase(), h = haystack.toLowerCase();
  let ni = 0, score = 0, run = 0, last = -1;
  for (let hi = 0; hi < h.length && ni < n.length; hi++) {
    if (h[hi] === n[ni]) {
      score += hi === 0 ? 10 : 1;
      score += last === hi - 1 ? ++run * 3 : (run = 0, 0);
      last = hi; ni++;
    }
  }
  return ni === n.length ? score : null;
}

export function fuzzyFilter(q: string, list: string[]): string[] {
  if (!q) return [...list];
  return list
    .map(s => ({ s, sc: fuzzyScore(q, s) }))
    .filter(x => x.sc !== null)
    .sort((a, b) => b.sc! - a.sc!)
    .map(x => x.s);
}

/** Best fuzzy score of `query` against any of `fields`, or null if none match. */
export function fuzzyMatch(query: string, fields: Array<string | null | undefined>): number | null {
  let best: number | null = null;
  for (const field of fields) {
    if (!field) continue;
    const score = fuzzyScore(query, field);
    if (score !== null && (best === null || score > best)) best = score;
  }
  return best;
}

/** Filters and ranks `items` by fuzzy-matching `query` against fields extracted via `getFields`. */
export function fuzzyFilterItems<T>(
  query: string,
  items: T[],
  getFields: (item: T) => Array<string | null | undefined>,
): T[] {
  if (!query) return items;
  return items
    .map(item => ({ item, score: fuzzyMatch(query, getFields(item)) }))
    .filter((x): x is { item: T; score: number } => x.score !== null)
    .sort((a, b) => b.score - a.score)
    .map(x => x.item);
}
