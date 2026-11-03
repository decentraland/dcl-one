import { catalystBase, type GetOptions } from "../client";
import { fetchEvents } from "./events";

type CalendarFilters = { search?: string; recurrent?: boolean; from: string; to: string };
type CalendarOptions = Pick<GetOptions, "base" | "signal">;
type Calendar = Awaited<ReturnType<typeof fetchEvents>>;

const TTL_MS = 30_000;
const MAX_ENTRIES = 8;
const entries = new Map<string, { expires: number; value: Calendar }>();

/** Public calendar rows tolerate 30s of staleness and never depend on an account.
 * Only completed valid responses are cached; concurrent request signals stay independent.
 */
export async function fetchEventCalendar(filters: CalendarFilters, options: CalendarOptions = {}): Promise<Calendar> {
  options.signal?.throwIfAborted();
  const now = Date.now();
  for (const [key, entry] of entries) if (entry.expires <= now) entries.delete(key);
  const key = JSON.stringify([catalystBase(options.base), filters.search ?? "", filters.recurrent ?? null, filters.from, filters.to]);
  const cached = entries.get(key);
  if (cached) {
    entries.delete(key);
    entries.set(key, cached);
    return cached.value;
  }
  const value = await fetchEvents({ ...filters, list: "active", limit: 500 }, options);
  options.signal?.throwIfAborted();
  if (!value.invalidCount) {
    if (entries.size >= MAX_ENTRIES) entries.delete(entries.keys().next().value!);
    entries.set(key, { expires: Date.now() + TTL_MS, value });
  }
  return value;
}
