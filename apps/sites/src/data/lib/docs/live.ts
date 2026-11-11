/** The English page a live English URL names, matched on its last segment (shared path segments break ties). */
export function englishMatch(liveUrl: string, pageUrls: string[]): string | null {
  const segs = liveUrl.replace(/^https?:\/\/[^/]+/, "").replace(/\.md$|\/+$/g, "").split("/").filter(Boolean);
  const last = segs[segs.length - 1];
  if (!last) return null;
  const hits = pageUrls.filter((u) => u.replace(/\/+$/, "").split("/").pop() === last);
  if (hits.length <= 1) return hits[0] ?? null;
  const score = (u: string) => u.split("/").filter((s) => segs.includes(s)).length;
  const ranked = [...hits].sort((a, b) => score(b) - score(a));
  return score(ranked[0]) > score(ranked[1]) ? ranked[0] : null;
}
