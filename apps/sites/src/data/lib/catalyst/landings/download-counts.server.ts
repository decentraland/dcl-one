// The public aggregate used by Sites, pinned in its production configuration at 5aabce35423428332c81a4a78a7a9c2b486446b5.
const COUNTS_URL =
  "https://cdn-data.decentraland.org/public/current/explorer-alpha-downloads.json";
export const DOWNLOADS_FALLBACK_LABEL = "+400K";

export function formatDownloads(total: number): string {
  if (total >= 1_000_000) return `+${Math.floor(total / 1_000_000)}M`;
  if (total >= 1_000) return `+${Math.floor(total / 1_000)}K`;
  return `+${total}`;
}

export async function fetchDownloadTotal(
  fetchImpl: typeof fetch = fetch,
): Promise<number | null> {
  try {
    const response = await fetchImpl(COUNTS_URL, {
      signal: AbortSignal.timeout(2000),
      headers: { accept: "application/json" },
    });
    if (!response.ok) {
      await response.body?.cancel().catch(() => undefined);
      return null;
    }
    const body: unknown = await response.json();
    if (
      !body ||
      typeof body !== "object" ||
      !("values" in body) ||
      !Array.isArray(body.values)
    )
      return null;
    let total = 0;
    for (const entry of body.values) {
      const count = Array.isArray(entry) ? entry[1] : undefined;
      if (
        typeof count !== "number" ||
        !Number.isSafeInteger(count) ||
        count < 0
      )
        return null;
      total += count;
      if (!Number.isSafeInteger(total)) return null;
    }
    return total > 0 ? total : null;
  } catch {
    return null;
  }
}

export function createDownloadsLabelLoader(
  readTotal: () => Promise<number | null> = fetchDownloadTotal,
  now: () => number = Date.now,
): () => Promise<string> {
  let lastGood: string | undefined;
  let refreshAfter = 0;
  let pending: Promise<void> | undefined;
  const refresh = () => {
    if (pending) return pending;
    pending = readTotal()
      .catch(() => null)
      .then((total) => {
        if (total !== null) lastGood = formatDownloads(total);
        refreshAfter = now() + (total === null ? 30000 : 600000);
      })
      .finally(() => {
        pending = undefined;
      });
    return pending;
  };
  return () => {
    if (now() < refreshAfter)
      return Promise.resolve(lastGood ?? DOWNLOADS_FALLBACK_LABEL);
    if (lastGood !== undefined) {
      void refresh();
      return Promise.resolve(lastGood);
    }
    return refresh().then(() => lastGood ?? DOWNLOADS_FALLBACK_LABEL);
  };
}

export const loadDownloadsLabel = createDownloadsLabelLoader();
