import { beforeEach, describe, expect, it, vi } from "vitest";
import { fetchEvents, type Event } from "../places/events";
import { fetchMostActivePlaces } from "../places/index";
import { heroEventCards } from "./home";
import { loadHomeEvents } from "./home.server";

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

describe("Home live-only event read", () => {
  it("requests the live feed once without fetching unused hotspots or rituals", async () => {
    vi.mocked(fetchEvents).mockResolvedValue({
      data: [event("live", true)],
    } as never);
    const result = await loadHomeEvents(opts);
    expect(fetchEvents).toHaveBeenCalledExactlyOnceWith(
      { list: "live", limit: 3 },
      opts,
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
});
