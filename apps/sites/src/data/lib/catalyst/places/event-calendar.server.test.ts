import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { EVENT_CALENDAR_LIMIT, fetchEvents, type Event } from "./events";
import { fetchEventCalendar } from "./event-calendar.server";

vi.mock("./events", async (original) => ({ ...await original<typeof import("./events")>(), fetchEvents: vi.fn() }));

describe("when reading the public event calendar", () => {
  let filters: Parameters<typeof fetchEventCalendar>[0];
  let options: Parameters<typeof fetchEventCalendar>[1];
  let result: Awaited<ReturnType<typeof fetchEvents>>;
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2099-06-01T12:00:00Z"));
    filters = { search: "music", from: "2099-06-01T00:00:00Z", to: "2099-06-29T00:00:00Z" };
    options = { base: `https://node.example/${encodeURIComponent(expect.getState().currentTestName ?? "")}` };
    result = { data: [{ id: "calendar-event" } as Event], total: 1, received: 1, invalidCount: 0 };
    vi.mocked(fetchEvents).mockResolvedValue(result);
  });
  afterEach(() => { vi.useRealTimers(); vi.resetAllMocks(); });

  it("should reuse a completed response across catalogue page views", async () => {
    await fetchEventCalendar(filters, options);
    await fetchEventCalendar(filters, options);
    expect(fetchEvents).toHaveBeenCalledTimes(1);
    expect(fetchEvents).toHaveBeenCalledWith({ ...filters, list: "active", limit: EVENT_CALENDAR_LIMIT }, { base: options?.base, signal: expect.any(AbortSignal) });
  });

  it("should isolate search, recurrence, date boundaries and catalogue bases", async () => {
    await fetchEventCalendar(filters, options);
    await fetchEventCalendar({ ...filters, search: "art" }, options);
    await fetchEventCalendar({ ...filters, recurrent: true }, options);
    await fetchEventCalendar({ ...filters, from: "2099-06-02T00:00:00Z" }, options);
    await fetchEventCalendar({ ...filters, to: "2099-06-08T00:00:00Z" }, options);
    await fetchEventCalendar(filters, { base: "https://other.example" });
    expect(fetchEvents).toHaveBeenCalledTimes(6);
  });

  it("should refresh after its documented 30-second freshness budget", async () => {
    await fetchEventCalendar(filters, options);
    vi.advanceTimersByTime(30_001);
    await fetchEventCalendar(filters, options);
    expect(fetchEvents).toHaveBeenCalledTimes(2);
  });

  it("should evict old search results rather than growing with every query", async () => {
    await fetchEventCalendar(filters, options);
    for (let index = 0; index < 20; index++) await fetchEventCalendar({ ...filters, search: `search-${index}` }, options);
    await fetchEventCalendar(filters, options);
    expect(fetchEvents).toHaveBeenCalledTimes(22);
  });

  describe("and the upstream request fails", () => {
    beforeEach(() => { vi.mocked(fetchEvents).mockRejectedValueOnce(new Error("offline")); });
    it("should let the next request retry immediately", async () => {
      await expect(fetchEventCalendar(filters, options)).rejects.toThrow("offline");
      await expect(fetchEventCalendar(filters, options)).resolves.toEqual(result);
      expect(fetchEvents).toHaveBeenCalledTimes(2);
    });
  });

  describe("and a parsed result contains invalid records", () => {
    beforeEach(() => { result.invalidCount = 1; });
    it("should retain the warning with cached valid rows until expiry", async () => {
      await fetchEventCalendar(filters, options);
      await expect(fetchEventCalendar(filters, options)).resolves.toMatchObject({ data: result.data, invalidCount: 1 });
      expect(fetchEvents).toHaveBeenCalledTimes(1);
      vi.advanceTimersByTime(30_001);
      vi.mocked(fetchEvents).mockResolvedValueOnce({ ...result, invalidCount: 0 });
      await expect(fetchEventCalendar(filters, options)).resolves.toHaveProperty("invalidCount", 0);
      expect(fetchEvents).toHaveBeenCalledTimes(2);
    });
  });

  describe("and several page requests need the same pending calendar", () => {
    let firstController: AbortController;
    let secondController: AbortController;
    let resolveShared: (result: Awaited<ReturnType<typeof fetchEvents>>) => void;
    let rejectShared: (reason: Error) => void;
    let first: Promise<Awaited<ReturnType<typeof fetchEventCalendar>>>;
    let second: Promise<Awaited<ReturnType<typeof fetchEventCalendar>>>;
    let sharedSignal: AbortSignal;
    beforeEach(() => {
      firstController = new AbortController();
      secondController = new AbortController();
      vi.mocked(fetchEvents).mockImplementationOnce((_params, upstream) => {
        sharedSignal = upstream!.signal!;
        return new Promise((resolve, reject) => { resolveShared = resolve; rejectShared = reject; });
      });
      first = fetchEventCalendar(filters, { ...options, signal: firstController.signal });
      second = fetchEventCalendar(filters, { ...options, signal: secondController.signal });
    });

    it("should coalesce concurrent reads and cache their completed result", async () => {
      expect(fetchEvents).toHaveBeenCalledTimes(1);
      expect(sharedSignal).not.toBe(firstController.signal);
      expect(sharedSignal).not.toBe(secondController.signal);
      resolveShared(result);
      await expect(Promise.all([first, second])).resolves.toEqual([result, result]);
      await expect(fetchEventCalendar(filters, options)).resolves.toEqual(result);
      expect(fetchEvents).toHaveBeenCalledTimes(1);
    });

    it("should cancel one wait without aborting another page's shared read", async () => {
      firstController.abort(new Error("page navigation cancelled"));
      await expect(first).rejects.toThrow("page navigation cancelled");
      expect(sharedSignal.aborted).toBe(false);
      resolveShared(result);
      await expect(second).resolves.toEqual(result);
      await expect(fetchEventCalendar(filters, options)).resolves.toEqual(result);
      expect(fetchEvents).toHaveBeenCalledTimes(1);
    });

    it("should keep a bounded public read reusable after all original waits cancel", async () => {
      firstController.abort(new Error("first page left"));
      secondController.abort(new Error("second page left"));
      await Promise.all([expect(first).rejects.toThrow("first page left"), expect(second).rejects.toThrow("second page left")]);
      expect(sharedSignal.aborted).toBe(false);
      resolveShared(result);
      await expect(fetchEventCalendar(filters, options)).resolves.toEqual(result);
      expect(fetchEvents).toHaveBeenCalledTimes(1);
    });

    it("should release a failed shared read so the next page can retry", async () => {
      rejectShared(new Error("offline"));
      await Promise.all([expect(first).rejects.toThrow("offline"), expect(second).rejects.toThrow("offline")]);
      await expect(fetchEventCalendar(filters, options)).resolves.toEqual(result);
      expect(fetchEvents).toHaveBeenCalledTimes(2);
    });

    it("should apply an independent deadline and never cache a late response", async () => {
      await Promise.all([
        expect(first).rejects.toMatchObject({ name: "TimeoutError" }),
        expect(second).rejects.toMatchObject({ name: "TimeoutError" }),
        vi.advanceTimersByTimeAsync(5001),
      ]);
      expect(sharedSignal.aborted).toBe(true);
      await expect(fetchEventCalendar(filters, options)).resolves.toEqual(result);
      resolveShared({ ...result, data: [{ id: "late" } as Event] });
      await expect(fetchEventCalendar(filters, options)).resolves.toEqual(result);
      expect(fetchEvents).toHaveBeenCalledTimes(2);
    });
  });

  describe("and many distinct calendars are already pending", () => {
    let reads: Promise<Awaited<ReturnType<typeof fetchEventCalendar>>>[];
    let resolvers: ((result: Awaited<ReturnType<typeof fetchEvents>>) => void)[];
    beforeEach(() => {
      resolvers = [];
      vi.mocked(fetchEvents).mockImplementation(() => new Promise((resolve) => { resolvers.push(resolve); }));
      reads = Array.from({ length: 8 }, (_, index) => fetchEventCalendar({ ...filters, search: `pending-${index}` }, options));
    });
    afterEach(async () => { for (const resolve of resolvers) resolve(result); await Promise.allSettled(reads); });
    it("should bound shared work and release capacity after completion", async () => {
      await expect(fetchEventCalendar(filters, options)).rejects.toThrow("The calendar is busy");
      expect(fetchEvents).toHaveBeenCalledTimes(8);
      resolvers[0](result);
      await reads[0];
      vi.mocked(fetchEvents).mockResolvedValueOnce(result);
      await expect(fetchEventCalendar(filters, options)).resolves.toEqual(result);
      expect(fetchEvents).toHaveBeenCalledTimes(9);
    });
  });
});
