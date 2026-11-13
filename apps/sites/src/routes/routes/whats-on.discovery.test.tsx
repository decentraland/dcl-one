import { renderToString } from "react-dom/server";
import { createMemoryRouter, RouterProvider, RouterContextProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { bucketEventsByDay, dayHeaderLabel, EVENT_CALENDAR_LIMIT, fetchEvents, relativeTimeLabel, toLiveNowCard, type Event } from "@/data/lib/catalyst/places/events";
import { fetchEventCalendar } from "@/data/lib/catalyst/places/event-calendar.server";
import { loadLiveNowCards, loadProfiles } from "@/data/lib/catalyst/places/live-now.server";
import WhatsOn, { loader } from "./whats-on";

const auth = vi.hoisted(() => ({ identity: null as object | null }));
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => ({ identity: auth.identity, isConnected: !!auth.identity }) }));
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));
vi.mock("@/data/lib/catalyst/places/events", async (original) => ({ ...await original<typeof import("@/data/lib/catalyst/places/events")>(), fetchEvents: vi.fn() }));
vi.mock("@/data/lib/catalyst/places/event-calendar.server", () => ({ fetchEventCalendar: vi.fn() }));
vi.mock("@/data/lib/catalyst/places/live-now.server", async (original) => ({ ...await original<typeof import("@/data/lib/catalyst/places/live-now.server")>(), loadLiveNowCards: vi.fn(), loadProfiles: vi.fn() }));

const NOW = new Date("2099-06-01T12:00:00Z");

describe("when the public event catalogue loads", () => {
  let events: Event[];
  let request: Request;
  let data: Extract<Awaited<ReturnType<typeof loader>>, { data: unknown }>["data"];
  let response: Extract<Awaited<ReturnType<typeof loader>>, { data: unknown }>;
  let renderPage: () => string;
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
    auth.identity = null;
    events = Array.from({ length: 12 }, (_, index) => ({ id: `event-${index}`, name: `Event ${index}`, start_at: "2099-06-01T18:00:00Z", finish_at: "2099-06-01T19:00:00Z", next_start_at: "2099-06-01T18:00:00Z", recurrent: false, approved: true, total_attendees: 90 } as Event));
    request = new Request("https://example.test/whats-on");
    vi.mocked(fetchEvents).mockImplementation(async () => ({ data: events.slice(0, 10), total: 10 }));
    vi.mocked(loadLiveNowCards).mockResolvedValue({ cards: [], copy: null });
    vi.mocked(loadProfiles).mockResolvedValue({});
    vi.mocked(fetchEventCalendar).mockResolvedValue({ data: events, total: events.length });
    renderPage = () => {
      const router = createMemoryRouter([{ id: "feed", path: "/whats-on", Component: WhatsOn as never }], { initialEntries: ["/whats-on"], hydrationData: { loaderData: { feed: data } } });
      return renderToString(<RouterProvider router={router} />).replace(/<!--.*?-->/g, "");
    };
  });
  afterEach(() => { vi.useRealTimers(); vi.restoreAllMocks(); vi.clearAllMocks(); });

  it("should request the next day of upcoming events and the ordered calendar", async () => {
    response = await loader({ request, context: new RouterContextProvider() } as never) as typeof response;
    expect(fetchEvents).toHaveBeenCalledWith({ list: "upcoming", limit: 10, order: "asc", from: "2099-06-01T12:00:00.000Z", to: "2099-06-02T12:00:00.000Z" }, expect.objectContaining({ signal: request.signal }));
    expect(loadLiveNowCards).toHaveBeenCalledWith(expect.objectContaining({ signal: request.signal }));
    expect(fetchEventCalendar).toHaveBeenCalledWith({ order: "asc" }, expect.anything());
    expect(response.data.upcoming).toHaveLength(10);
    expect(response.data.allDays[0]).toHaveLength(12);
    expect(response.data.dayLabels.slice(0, 3)).toEqual(["Today", "Tomorrow", "Wed, Jun 3"]);
  });

  it("should show the All Events heading signed out and the tabs signed in", async () => {
    response = await loader({ request, context: new RouterContextProvider() } as never) as typeof response;
    data = response.data;
    expect(renderPage()).toContain('<h2 class="wo-all__title">All Events</h2>');
    auth.identity = {};
    expect(renderPage()).toContain('href="/whats-on?tab=my"');
  });

  describe("and the API is unavailable", () => {
    beforeEach(() => { vi.mocked(fetchEvents).mockRejectedValue(new Error("offline")); vi.mocked(fetchEventCalendar).mockRejectedValue(new Error("offline")); vi.mocked(loadLiveNowCards).mockRejectedValue(new Error("offline")); });
    it("should show a retryable failure instead of an empty catalogue", async () => {
      response = await loader({ request, context: new RouterContextProvider() } as never) as typeof response;
      data = response.data;
      const html = renderPage();
      expect(html).toContain("Events are temporarily unavailable");
      expect(html).toContain("Retry");
      expect(response.init?.status).toBe(503);
    });
  });

  describe("and only the live feed fails", () => {
    beforeEach(() => { vi.mocked(loadLiveNowCards).mockRejectedValue(new Error("offline")); });
    it("should keep the calendar and offer a retry for the missing feed", async () => {
      response = await loader({ request, context: new RouterContextProvider() } as never) as typeof response;
      data = response.data;
      expect(renderPage()).toContain("Live events are temporarily unavailable");
      expect(response.init?.status).toBe(200);
    });
  });

  describe("and malformed data is the only problem", () => {
    beforeEach(() => { vi.mocked(fetchEventCalendar).mockResolvedValue({ data: events, total: events.length, invalidCount: 1 }); });
    it("should show the data warning without offering a service retry", async () => {
      response = await loader({ request, context: new RouterContextProvider() } as never) as typeof response;
      data = response.data;
      const html = renderPage();
      expect(html).toContain("Some events could not be displayed");
      expect(html).not.toContain(">Retry<");
    });
  });

  describe("and the calendar read is full", () => {
    beforeEach(() => { vi.mocked(fetchEventCalendar).mockResolvedValue({ data: events, total: events.length, received: EVENT_CALENDAR_LIMIT }); });
    it("should say that later days can be incomplete", async () => {
      response = await loader({ request, context: new RouterContextProvider() } as never) as typeof response;
      data = response.data;
      expect(data.calendarLimited).toBe(true);
      expect(renderPage()).toContain(`The calendar shows the first ${EVENT_CALENDAR_LIMIT} events`);
    });
  });

  describe("and the creators have profiles", () => {
    beforeEach(() => {
      events[0] = { ...events[0], user: "0xAA", user_name: "alias" };
      events[1] = { ...events[1], user: "0xBB", user_name: "Decentraland Foundation" };
      vi.mocked(loadLiveNowCards).mockResolvedValue({ cards: [{ id: "s1", title: "Club", users: 12, image: "", coordinates: "1,2", creatorAddress: "0xCC", creatorName: "MetaDoge", isGenesisPlaza: false, place: true }], copy: null });
      vi.mocked(loadProfiles).mockResolvedValue({ "0xaa": { address: "0xaa", name: "Maker", hasClaimedName: true, face: "/face-aa.png" }, "0xcc": { address: "0xcc", name: "Doge", hasClaimedName: true, face: "/face-cc.png" } });
    });
    it("should read the faces per card group and keep the Foundation's logo", async () => {
      response = await loader({ request, context: new RouterContextProvider() } as never) as typeof response;
      data = response.data;
      expect(vi.mocked(loadProfiles).mock.calls.map(([ids]) => ids)).toEqual([["0xCC", "0xAA", ...events.slice(2, 10).flatMap((e) => (e.user ? [e.user] : []))], ["0xAA"]]);
      expect(data.live[0]).toMatchObject({ creator: "MetaDoge", face: "/face-cc.png", isEvent: false });
      expect(data.upcoming[0]).toMatchObject({ creator: "Maker", face: "/face-aa.png" });
      expect(data.upcoming[1]).toMatchObject({ creator: "Decentraland Foundation", color: "#9d76e3" });
      expect(data.allDays[0][0]).toMatchObject({ creator: "Maker", face: "/face-aa.png" });
      expect(renderPage()).toContain('<img class="u-avatar__img" src="/face-aa.png" alt=""/>');
    });
  });

  describe("and a live event has many RSVPs", () => {
    it("should leave live occupancy unknown without a presence observation", () => {
      expect(toLiveNowCard(events[0]).users).toBeNull();
    });
    it("should keep a successful empty presence observation distinct from unknown", () => {
      expect(toLiveNowCard({ ...events[0], connected_addresses: [] }).users).toBe(0);
    });
    it("should count observed connected wallets independently of RSVPs", () => {
      expect(toLiveNowCard({ ...events[0], connected_addresses: ["0xAb", "0xab", "0xcd"] }).users).toBe(2);
    });
  });
});

describe("when events are placed on the calendar", () => {
  const base = { id: "e", name: "E", approved: true, rejected: false, live: false, total_attendees: 0 } as Event;

  it("should put each occurrence of a recurring event on its day", () => {
    const days = bucketEventsByDay([{ ...base, recurrent: true, duration: 3_600_000, start_at: "2099-01-01T10:00:00Z", recurrent_dates: ["2099-06-01T15:00:00Z", "2099-06-02T15:00:00Z"] }], 3, NOW);
    expect(days.map((day) => day.length)).toEqual([1, 1, 0]);
    expect(days[0][0].time).toBe("3:00 PM");
  });

  it("should drop an occurrence that already ended and events nobody approved", () => {
    const days = bucketEventsByDay([
      { ...base, id: "ended", start_at: "2099-06-01T08:00:00Z", finish_at: "2099-06-01T09:00:00Z" },
      { ...base, id: "pending", approved: false, start_at: "2099-06-01T18:00:00Z", finish_at: "2099-06-01T19:00:00Z" },
      { ...base, id: "later", start_at: "2099-06-01T18:00:00Z", finish_at: "2099-06-01T19:00:00Z" },
      { ...base, id: "sooner", start_at: "2099-06-01T13:00:00Z", finish_at: "2099-06-01T14:00:00Z" },
    ], 1, NOW);
    expect(days[0].map((event) => event.id)).toEqual(["sooner", "later"]);
  });

  it("should keep a live event that started before midnight on today", () => {
    const days = bucketEventsByDay([
      { ...base, id: "overnight", live: true, start_at: "2099-05-31T22:00:00Z", finish_at: "2099-06-01T14:00:00Z" },
      { ...base, id: "yesterday", start_at: "2099-05-31T08:00:00Z", finish_at: "2099-05-31T09:00:00Z" },
    ], 2, NOW);
    expect(days[0].map((event) => event.id)).toEqual(["overnight"]);
    expect(days[0][0].live).toBe(true);
  });

  it("should label times relative to now", () => {
    expect(relativeTimeLabel("2099-06-01T12:20:00Z", NOW)).toBe("Starts in 20 mins");
    expect(relativeTimeLabel("2099-06-01T18:00:00Z", NOW)).toBe("Today 6:00 PM");
    expect(relativeTimeLabel("2099-06-02T01:00:00Z", NOW)).toBe("Tomorrow 1:00 AM");
    expect(dayHeaderLabel(4, NOW)).toBe("Fri, Jun 5");
  });
});
