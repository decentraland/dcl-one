import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fetchEvents, type Event } from "../places/events";
import { fetchMostActivePlaces } from "../places/index";
import type { Place } from "../schema";
import { withCreators } from "./events.server";
import { loadHome } from "./home.server";

vi.mock("../places/events", async (original) => {
  const actual = await original<typeof import("../places/events")>();
  return { ...actual, fetchEvents: vi.fn(actual.fetchEvents) };
});
vi.mock("../places/index", async (original) => {
  const actual = await original<typeof import("../places/index")>();
  return {
    ...actual,
    fetchMostActivePlaces: vi.fn(actual.fetchMostActivePlaces),
  };
});
vi.mock("./events.server", async (original) => {
  const actual = await original<typeof import("./events.server")>();
  return { ...actual, withCreators: vi.fn(actual.withCreators) };
});
vi.mock("./download-counts.server", async (original) => {
  const actual = await original<typeof import("./download-counts.server")>();
  return {
    ...actual,
    loadDownloadsLabel: vi.fn(async () => actual.DOWNLOADS_FALLBACK_LABEL),
  };
});

const place = (extra: Partial<Place> = {}): Place =>
  ({
    id: "synthetic-place",
    title: "Synthetic place",
    image: "https://images.example/synthetic-place.png",
    owner: null,
    positions: ["1,1"],
    base_position: "1,1",
    contact_name: null,
    user_count: 8,
    world: false,
    ...extra,
  }) as Place;
const event = (extra: Partial<Event> = {}): Event =>
  ({
    id: "synthetic-event",
    name: "Synthetic event",
    image: "https://images.example/synthetic-event.png",
    x: 1,
    y: 1,
    live: true,
    ...extra,
  }) as Event;
const opts = { base: "http://synthetic-node.example" };

describe("when loading home discovery from the configured node", () => {
  afterEach(() => vi.clearAllMocks());
  beforeEach(() => {
    vi.mocked(fetchEvents)
      .mockReset()
      .mockResolvedValue({ data: [], total: 0 });
    vi.mocked(fetchMostActivePlaces).mockReset().mockResolvedValue([]);
    vi.mocked(withCreators)
      .mockReset()
      .mockImplementation(async (cards) => cards);
  });

  it("should return no invented cards or creator lookups when all discovery reads fail", async () => {
    vi.mocked(fetchEvents).mockRejectedValue(new Error("Node unavailable"));
    vi.mocked(fetchMostActivePlaces).mockRejectedValue(
      new Error("Node unavailable"),
    );
    const home = await loadHome(opts);
    expect(home.whatsOn).toEqual([]);
    expect([...home.unreadable].sort()).toEqual([
      "events",
      "hotspots",
      "live-events",
      "rituals",
    ]);
    expect(withCreators).not.toHaveBeenCalled();
  });

  it("should keep successful empty lists readable without creator lookups", async () => {
    const home = await loadHome(opts);
    expect(home.whatsOn).toEqual([]);
    expect(home.unreadable).toEqual([]);
    expect(withCreators).not.toHaveBeenCalled();
  });

  it("should require supplied places even when live events are available", async () => {
    vi.mocked(fetchMostActivePlaces).mockRejectedValue(
      new Error("Places unavailable"),
    );
    vi.mocked(fetchEvents).mockImplementation(async (params) => ({
      data: params?.list === "live" ? [event()] : [],
      total: params?.list === "live" ? 1 : 0,
    }));
    const home = await loadHome(opts);
    expect(home.whatsOn).toEqual([]);
    expect(home.unreadable).toEqual(["hotspots"]);
    expect(withCreators).not.toHaveBeenCalled();
  });

  it("should expose a failed live list without discarding actual supplied place data", async () => {
    vi.mocked(fetchMostActivePlaces).mockResolvedValue([place()]);
    vi.mocked(fetchEvents).mockImplementation(async (params) => {
      if (params?.list === "live") throw new Error("Live list unavailable");
      return { data: [], total: 0 };
    });
    const home = await loadHome(opts);
    expect(home.unreadable).toEqual(["live-events"]);
    expect(home.whatsOn).toHaveLength(1);
    expect(home.whatsOn[0]).toMatchObject({
      id: "synthetic-place",
      type: "place",
      image: "https://images.example/synthetic-place.png",
      users: 8,
    });
    expect(withCreators).toHaveBeenCalledTimes(1);
  });

  it("should preserve a supplied Genesis place's actual identity and image", async () => {
    vi.mocked(fetchMostActivePlaces).mockResolvedValue([
      place({
        id: "synthetic-genesis",
        title: "Supplied Genesis",
        image: "https://images.example/supplied-genesis.png",
        user_count: 0,
        base_position: "0,0",
        positions: ["0,0"],
      }),
    ]);
    const home = await loadHome(opts);
    expect(home.whatsOn).toHaveLength(1);
    expect(home.whatsOn[0]).toMatchObject({
      id: "synthetic-genesis",
      title: "Supplied Genesis",
      users: 0,
      image: "https://images.example/supplied-genesis.png",
      isGenesisPlaza: true,
      href: "/places/synthetic-genesis",
    });
  });

  it("should not invent Genesis when only low-occupancy non-Genesis places are supplied", async () => {
    vi.mocked(fetchMostActivePlaces).mockResolvedValue([
      place({ user_count: 0 }),
    ]);
    const home = await loadHome(opts);
    expect(home.whatsOn).toEqual([]);
    expect(home.unreadable).toEqual([]);
    expect(withCreators).not.toHaveBeenCalled();
  });

  it("should retain live-event priority and event imagery for a supplied crowded place", async () => {
    vi.mocked(fetchMostActivePlaces).mockResolvedValue([place()]);
    vi.mocked(fetchEvents).mockImplementation(async (params) => ({
      data: params?.list === "live" ? [event()] : [],
      total: params?.list === "live" ? 1 : 0,
    }));
    const home = await loadHome(opts);
    expect(home.whatsOn).toHaveLength(1);
    expect(home.whatsOn[0]).toMatchObject({
      type: "event",
      id: "synthetic-event",
      image: "https://images.example/synthetic-event.png",
      users: 8,
    });
  });

  it("should retry failed live data instead of retaining it as an empty memoized success", async () => {
    vi.mocked(fetchEvents).mockImplementation(async (params) => {
      if (params?.list === "live") throw new Error("Live list unavailable");
      return { data: [], total: 0 };
    });
    expect((await loadHome()).unreadable).toEqual(["live-events"]);
    vi.mocked(fetchEvents).mockResolvedValue({ data: [], total: 0 });
    expect((await loadHome()).unreadable).toEqual([]);
    expect(
      vi
        .mocked(fetchEvents)
        .mock.calls.filter(([params]) => params?.list === "live"),
    ).toHaveLength(2);
  });
});
