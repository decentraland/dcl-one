import { z } from "zod";

import { downloadCountsUrl } from "../catalyst/bases";
import { ttlMemo } from "../ttl-memo";

// The Explorer's download total,
// read from DOWNLOAD_COUNTS_URL (a JSON of [platform, downloads] pairs) and summed into "+438K". Unset,
// or never read, the total is null: the hero shows its fallback and /download hides the line, as the
// reference does with an empty answer; no number is made up.

const DOWNLOAD_COUNTS_TTL_MS = 60_000;

const CountsSchema = z.object({ values: z.array(z.tuple([z.string(), z.number()])) });

export function formatToShorthand(num: number): string {
  if (num >= 1_000_000) return `+${Math.floor(num / 1_000_000)}M`;
  if (num >= 1_000) return `+${Math.floor(num / 1_000)}K`;
  return `+${num}`;
}

// Both document loaders that show the total wait for this read, and DOWNLOAD_COUNTS_URL is another
// operator's host. 2 s is the bound the loader this one replaced had: past it the page renders with
// the last total read, or without one.
const DOWNLOAD_COUNTS_TIMEOUT_MS = 2_000;
// A failed read is held for 30 s, as that loader did, so one bad read does not repeat for each request.
const DOWNLOAD_COUNTS_FAILURE_TTL_MS = 30_000;

type Counts = { label: string | null; read: boolean };

let lastRead: string | null = null;

async function readDownloadCounts(url: string): Promise<string | null> {
  const response = await fetch(url, {
    signal: AbortSignal.timeout(DOWNLOAD_COUNTS_TIMEOUT_MS),
    headers: { accept: "application/json" },
  });
  if (!response.ok) {
    await response.body?.cancel().catch(() => undefined);
    return null;
  }
  const parsed = CountsSchema.safeParse(await response.json());
  if (!parsed.success) return null;
  const total = parsed.data.values.reduce((sum, [, downloads]) => sum + downloads, 0);
  return total > 0 ? formatToShorthand(total) : null;
}

async function fetchDownloadCounts(): Promise<Counts> {
  const url = downloadCountsUrl();
  if (!url) return { label: null, read: true };
  const label = await readDownloadCounts(url).catch(() => null);
  if (label === null) return { label: lastRead, read: false };
  lastRead = label;
  return { label, read: true };
}

const countsMemo = ttlMemo({
  ttlMs: DOWNLOAD_COUNTS_TTL_MS,
  ttlMsForValue: (counts: Counts) => (counts.read ? DOWNLOAD_COUNTS_TTL_MS : DOWNLOAD_COUNTS_FAILURE_TTL_MS),
  // An expired total serves while one reload runs behind it, so a visitor waits only when the memo is cold.
  staleMs: DOWNLOAD_COUNTS_TTL_MS,
  load: fetchDownloadCounts,
});

/**
 * The shorthand total, visitor-independent, read once a minute. After a failed read it is the last total read,
 * or null when none was.
 */
export async function loadDownloadCounts(): Promise<string | null> {
  return (await countsMemo()).label;
}

export function resetDownloadCounts(): void {
  lastRead = null;
  countsMemo.reset();
}
