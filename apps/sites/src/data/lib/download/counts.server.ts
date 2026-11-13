import { z } from "zod";

import { downloadCountsUrl } from "../catalyst/bases";
import { ttlMemo } from "../ttl-memo";

// from: sites@17b3535a:src/modules/explorerDownloads.ts and src/modules/number.ts: the Explorer's download total,
// read from DOWNLOAD_COUNTS_URL (a JSON of [platform, downloads] pairs) and summed into "+438K". Unset,
// unreachable or unreadable, the total is null: the hero shows its fallback and /download hides the line, as
// the reference does with an empty answer; no number is made up.

const DOWNLOAD_COUNTS_TTL_MS = 60_000;

const CountsSchema = z.object({ values: z.array(z.tuple([z.string(), z.number()])) });

export function formatToShorthand(num: number): string {
  if (num >= 1_000_000) return `+${Math.floor(num / 1_000_000)}M`;
  if (num >= 1_000) return `+${Math.floor(num / 1_000)}K`;
  return `+${num}`;
}

async function fetchDownloadCounts(): Promise<string | null> {
  const url = downloadCountsUrl();
  if (!url) return null;
  try {
    const response = await fetch(url);
    if (!response.ok) return null;
    const parsed = CountsSchema.safeParse(await response.json());
    if (!parsed.success) return null;
    const total = parsed.data.values.reduce((sum, [, downloads]) => sum + downloads, 0);
    return total > 0 ? formatToShorthand(total) : null;
  } catch {
    return null;
  }
}

const countsMemo = ttlMemo({ ttlMs: DOWNLOAD_COUNTS_TTL_MS, keep: (v: string | null) => v !== null, load: fetchDownloadCounts });

/** The shorthand total, visitor-independent, read once a minute; null until a read succeeds. */
export function loadDownloadCounts(): Promise<string | null> {
  return countsMemo();
}

export const resetDownloadCounts = countsMemo.reset;
