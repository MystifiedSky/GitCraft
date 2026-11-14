/** Stable bounded ranking, retaining the original order for equal scores. */
export function topMatches<T>(items: readonly T[], score: (item: T) => number, limit = 80): T[] {
  const best: { item: T; score: number }[] = [];
  for (const item of items) {
    const value = score(item);
    if (value <= 0 || (best.length >= limit && value <= best[best.length - 1].score)) continue;
    let low = 0, high = best.length;
    while (low < high) { const mid = (low + high) >>> 1; if (best[mid].score >= value) low = mid + 1; else high = mid; }
    best.splice(low, 0, { item, score: value });
    if (best.length > limit) best.pop();
  }
  return best.map(entry => entry.item);
}
