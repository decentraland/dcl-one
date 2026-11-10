import { ttlMemo } from "../../ttl-memo";

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

const counts = ttlMemo({
  ttlMs: 600000,
  keep: (total: number | null) => total !== null,
  load: () => fetchDownloadTotal(),
});
export async function loadDownloadsLabel(): Promise<string> {
  const total = await counts();
  return total === null ? DOWNLOADS_FALLBACK_LABEL : formatDownloads(total);
}
