import { ttlMemo } from "../../ttl-memo";
import { downloadCountsUrl } from "../bases";

// The Explorer download total the home shows (source Sites modules/explorerDownloads.ts): a list of
// [platform, downloads] pairs whose sum is the counter. Decorative, so every failure shows the
// source's static label instead.

export const DOWNLOADS_FALLBACK_LABEL = "+400K";
const COUNTS_TIMEOUT_MS = 2_000;
const COUNTS_TTL_MS = 10 * 60_000;

/** Source Sites modules/number.ts formatToShorthand. */
export function formatDownloads(total: number): string {
  if (total >= 1_000_000) return `+${Math.floor(total / 1_000_000)}M`;
  if (total >= 1_000) return `+${Math.floor(total / 1_000)}K`;
  return `+${total}`;
}

/** The summed downloads, or null when the endpoint is unset, slow, failing or malformed. */
export async function fetchDownloadTotal(
  url: string | undefined,
  fetchImpl: typeof fetch = fetch,
  timeoutMs = COUNTS_TIMEOUT_MS,
): Promise<number | null> {
  if (!url) return null;
  try {
    const res = await fetchImpl(url, {
      signal: AbortSignal.timeout(timeoutMs),
      headers: { accept: "application/json" },
    });
    if (!res.ok) return null;
    const body = (await res.json()) as { values?: unknown };
    if (!Array.isArray(body.values)) return null;
    let total = 0;
    for (const entry of body.values) {
      const downloads = Array.isArray(entry) ? entry[1] : undefined;
      if (
        typeof downloads !== "number" ||
        !Number.isFinite(downloads) ||
        downloads < 0
      )
        return null;
      total += downloads;
    }
    return total > 0 ? total : null;
  } catch {
    return null;
  }
}

const countsMemo = ttlMemo({
  ttlMs: COUNTS_TTL_MS,
  keep: (total: number | null) => total !== null,
  load: () => fetchDownloadTotal(downloadCountsUrl()),
});

/** The counter label, memoized for every visitor; a failed read is retried on the next request. */
export async function loadDownloadsLabel(): Promise<string> {
  const total = await countsMemo();
  return total === null ? DOWNLOADS_FALLBACK_LABEL : formatDownloads(total);
}

export { countsMemo as downloadCountsMemo };
