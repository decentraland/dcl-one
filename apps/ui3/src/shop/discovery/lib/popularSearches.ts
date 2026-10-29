// from: shop@94584a5c:app/src/lib/popularSearches.ts
export const POPULAR_SEARCHES = ["duck", "sword", "kimono", "wings", "dance", "hoodie", "dress", "glow"]

/** The popular searches a reader has not already got among their recent ones, case aside. */
export function popularSearchesFor(recent: string[]): string[] {
  const seen = new Set(recent.map(term => term.trim().toLowerCase()))
  return POPULAR_SEARCHES.filter(term => !seen.has(term))
}
