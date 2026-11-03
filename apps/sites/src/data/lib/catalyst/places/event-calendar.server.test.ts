import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fetchEvents, type Event } from "./events";
import { fetchEventCalendar } from "./event-calendar.server";

vi.mock("./events", () => ({ fetchEvents: vi.fn() }));

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
    expect(fetchEvents).toHaveBeenCalledWith({ ...filters, list: "active", limit: 500 }, options);
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

  describe("and a result contains invalid records", () => {
    beforeEach(() => { result.invalidCount = 1; });
    it("should not retain damaged data after the service repairs it", async () => {
      await fetchEventCalendar(filters, options);
      await fetchEventCalendar(filters, options);
      expect(fetchEvents).toHaveBeenCalledTimes(2);
    });
  });

  describe("and concurrent page requests have independent cancellation", () => {
    let cancelled: AbortController;
    let resolveCancelled: (result: Awaited<ReturnType<typeof fetchEvents>>) => void;
    let resolveOther: (result: Awaited<ReturnType<typeof fetchEvents>>) => void;
    let first: Promise<Awaited<ReturnType<typeof fetchEventCalendar>>>;
    let second: Promise<Awaited<ReturnType<typeof fetchEventCalendar>>>;
    beforeEach(() => {
      cancelled = new AbortController();
      vi.mocked(fetchEvents).mockImplementationOnce(() => new Promise((resolve) => { resolveCancelled = resolve; }));
      vi.mocked(fetchEvents).mockImplementationOnce(() => new Promise((resolve) => { resolveOther = resolve; }));
      first = fetchEventCalendar(filters, { ...options, signal: cancelled.signal });
      second = fetchEventCalendar(filters, options);
    });
    it("should not cancel another request or cache a cancelled result", async () => {
      cancelled.abort(new Error("page navigation cancelled"));
      resolveCancelled({ ...result, data: [{ id: "cancelled-result" } as Event] });
      resolveOther(result);
      await expect(first).rejects.toThrow("page navigation cancelled");
      await expect(second).resolves.toEqual(result);
      await expect(fetchEventCalendar(filters, options)).resolves.toEqual(result);
      expect(fetchEvents).toHaveBeenCalledTimes(2);
    });
  });
});
