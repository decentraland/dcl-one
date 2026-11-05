import { catalystBase, type GetOptions } from "../client";
import { EVENT_CALENDAR_LIMIT, fetchEvents } from "./events";

type CalendarFilters = { search?: string; recurrent?: boolean; from: string; to: string };
type CalendarOptions = Pick<GetOptions, "base" | "signal">;
type Calendar = Awaited<ReturnType<typeof fetchEvents>>;

const TTL_MS = 30_000;
const MAX_ENTRIES = 8;
const READ_TIMEOUT_MS = 5_000;
const entries = new Map<string, { expires: number; value: Calendar }>();
const inFlight = new Map<string, Promise<Calendar>>();

function awaitCalendar(pending: Promise<Calendar>, signal?: AbortSignal): Promise<Calendar> {
  signal?.throwIfAborted();
  if (!signal) return pending;
  return new Promise((resolve, reject) => {
    const cancelled = () => reject(signal.reason);
    signal.addEventListener("abort", cancelled, { once: true });
    pending.then(
      (value) => { signal.removeEventListener("abort", cancelled); resolve(value); },
      (error: unknown) => { signal.removeEventListener("abort", cancelled); reject(error); },
    );
  });
}

function readCalendar(key: string, filters: CalendarFilters, base?: string): Promise<Calendar> {
  const controller = new AbortController();
  let timer: ReturnType<typeof setTimeout>;
  const deadline = new Promise<Calendar>((_resolve, reject) => {
    timer = setTimeout(() => {
      const error = new DOMException("The calendar read exceeded its time budget.", "TimeoutError");
      controller.abort(error);
      reject(error);
    }, READ_TIMEOUT_MS);
  });
  const pending = Promise.race([
    fetchEvents({ ...filters, list: "active", limit: EVENT_CALENDAR_LIMIT }, { base, signal: controller.signal }),
    deadline,
  ]).then((value) => {
    controller.signal.throwIfAborted();
    if (entries.size >= MAX_ENTRIES) entries.delete(entries.keys().next().value!);
    entries.set(key, { expires: Date.now() + TTL_MS, value });
    return value;
  }).finally(() => {
    clearTimeout(timer);
    inFlight.delete(key);
  });
  inFlight.set(key, pending);
  // A completed public read can still populate the cache after all callers leave.
  void pending.catch(() => {});
  return pending;
}

/** Public calendars share bounded reads and 30s of staleness, never an account.
 * Malformed-row warnings stay with the parsed response; request cancellation
 * stops only that caller's wait, while the shared read has its own deadline.
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
  let pending = inFlight.get(key);
  if (!pending) {
    if (inFlight.size >= MAX_ENTRIES) throw new Error("The calendar is busy. Please retry shortly.");
    pending = readCalendar(key, filters, options.base);
  }
  return awaitCalendar(pending, options.signal);
}
