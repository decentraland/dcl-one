import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fetchEvents, type Event } from "../places/events";
import { fetchMostActivePlaces } from "../places/index";
import { heroEventCards } from "./home";
import { createHomeEventsLoader, loadHomeEvents } from "./home.server";

vi.mock("../places/events", async (original) => ({
  ...(await original<typeof import("../places/events")>()),
  fetchEvents: vi.fn(),
}));
vi.mock("../places/index", async (original) => ({
  ...(await original<typeof import("../places/index")>()),
  fetchMostActivePlaces: vi.fn(),
}));
const event = (id: string, live: boolean): Event =>
  ({
    id,
    live,
    name: id,
    image: null,
    start_at: null,
    finish_at: null,
    next_start_at: null,
    all_day: false,
  }) as Event;
const opts = { fetchImpl: vi.fn() };
beforeEach(() => vi.resetAllMocks());
afterEach(() => vi.useRealTimers());

describe("Home live-only event read", () => {
  it("requests the live feed once without fetching unused hotspots or rituals", async () => {
    vi.mocked(fetchEvents).mockResolvedValue({
      data: [event("live", true)],
    } as never);
    const result = await loadHomeEvents(opts);
    expect(fetchEvents).toHaveBeenCalledOnce();
    expect(fetchEvents).toHaveBeenCalledWith(
      { list: "live", limit: 3 },
      expect.objectContaining({
        fetchImpl: opts.fetchImpl,
        signal: expect.any(AbortSignal),
      }),
    );
    expect(fetchMostActivePlaces).not.toHaveBeenCalled();
    expect(heroEventCards(result.content)?.map((card) => card.id)).toEqual([
      "live",
    ]);
  });
  it("filters before the display cap rather than hiding live events after upcoming items", async () => {
    vi.mocked(fetchEvents).mockResolvedValue({
      data: [
        event("upcoming1", false),
        event("upcoming2", false),
        event("upcoming3", false),
        event("live", true),
      ],
    } as never);
    expect(
      heroEventCards((await loadHomeEvents(opts)).content)?.map(
        (card) => card.id,
      ),
    ).toEqual(["live"]);
  });
  it("keeps content available for markdown but no invented cards for empty or failed feeds", async () => {
    vi.mocked(fetchEvents)
      .mockResolvedValueOnce({ data: [] } as never)
      .mockRejectedValueOnce(new Error("offline"));
    const empty = await loadHomeEvents(opts);
    const failed = await loadHomeEvents(opts);
    expect(empty.content.hero.title).toBeTruthy();
    expect(heroEventCards(empty.content)).toBeUndefined();
    expect(heroEventCards(failed.content)).toBeUndefined();
    expect(failed.unreadable).toEqual(["events"]);
  });

  it("aborts a hanging events request at the deadline", async () => {
    vi.useFakeTimers();
    vi.mocked(fetchEvents).mockImplementation(
      (_params, options) =>
        new Promise((_resolve, reject) => {
          options?.signal?.addEventListener(
            "abort",
            () => reject(options.signal?.reason),
            { once: true },
          );
        }),
    );

    const pending = loadHomeEvents({ fetchImpl: vi.fn() });
    await vi.advanceTimersByTimeAsync(2000);
    const result = await pending;

    expect(vi.mocked(fetchEvents).mock.calls[0]?.[1]?.signal?.aborted).toBe(
      true,
    );
    expect(result.unreadable).toEqual(["events"]);
  });

  it("caches an unavailable live rail briefly before retrying", async () => {
    vi.useFakeTimers();
    vi.mocked(fetchEvents)
      .mockRejectedValueOnce(new Error("offline"))
      .mockResolvedValue({ data: [event("live", true)] } as never);
    const load = createHomeEventsLoader(fetchEvents);

    const first = await load();
    const cached = await load();
    expect(first.unreadable).toEqual(["events"]);
    expect(cached.unreadable).toEqual(["events"]);
    expect(fetchEvents).toHaveBeenCalledOnce();

    await vi.advanceTimersByTimeAsync(5000);
    const recovered = await load();
    expect(recovered.live).toBe(true);
    expect(recovered.unreadable).toEqual([]);
    expect(fetchEvents).toHaveBeenCalledTimes(2);
  });
});
