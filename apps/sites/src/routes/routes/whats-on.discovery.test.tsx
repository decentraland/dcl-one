import { renderToString } from "react-dom/server";
import { createMemoryRouter, RouterProvider, RouterContextProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fetchEvents, fetchEventPage, toLiveNowCard, type Event } from "@/data/lib/catalyst/places/events";
import { fetchEventCalendar } from "@/data/lib/catalyst/places/event-calendar.server";
import WhatsOn, { loader } from "./whats-on";

vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => ({ identity: null, isConnected: false }) }));
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));
vi.mock("@/data/lib/catalyst/places/events", async (original) => ({ ...await original<typeof import("@/data/lib/catalyst/places/events")>(), fetchEvents: vi.fn(), fetchEventPage: vi.fn() }));

vi.mock("@/data/lib/catalyst/places/event-calendar.server", () => ({ fetchEventCalendar: vi.fn() }));

describe("when the public hangout catalogue is paginated", () => {
  let events: Event[];
  let request: Request;
  let data: Extract<Awaited<ReturnType<typeof loader>>, { data: unknown }>["data"];
  let html: string;
  let response: Extract<Awaited<ReturnType<typeof loader>>, { data: unknown }>;
  let renderPage: () => string;
  beforeEach(() => {
    events = Array.from({ length: 25 }, (_, index) => ({ id: `event-${index}`, name: `Hangout ${index}`, start_at: "2099-06-01T18:00:00Z", next_start_at: "2099-06-01T18:00:00Z", recurrent: true, total_attendees: 90 } as Event));
    request = new Request("https://example.test/whats-on?search=music&offset=120&filter=recurring");
    vi.mocked(fetchEvents).mockImplementation(async (params) => ({ data: params?.list === "live" ? [] : events, total: events.length }));
    vi.mocked(fetchEventPage).mockResolvedValue({ data: events.slice(0, 24), hasMore: true });
    vi.mocked(fetchEventCalendar).mockResolvedValue({ data: events, total: events.length });
    renderPage = () => {
      const router = createMemoryRouter([{ id: "feed", path: "/whats-on", Component: WhatsOn as never }], { initialEntries: [request.url.replace("https://example.test", "")], hydrationData: { loaderData: { feed: data } } });
      return renderToString(<RouterProvider router={router} />).replace(/<!--.*?-->/g, "");
    };
  });
  afterEach(() => { vi.restoreAllMocks(); vi.clearAllMocks(); });

  it("should request the selected page and apply recurrence and search before pagination", async () => {
    response = await loader({ request, context: new RouterContextProvider() } as never) as typeof response;
    data = response.data;
    expect(fetchEventPage).toHaveBeenCalledWith(expect.objectContaining({ list: "active", offset: 120, limit: 24, search: "music", recurrent: true }), expect.objectContaining({ signal: request.signal }));
    expect(fetchEvents).toHaveBeenCalledWith(expect.objectContaining({ list: "live", recurrent: true, with_connected_users: true }), expect.anything());
    expect(data.upcoming).toHaveLength(24);
    expect(data.hasMore).toBe(true);
  });

  it("should expose search and next/previous links retaining filters", async () => {
    response = await loader({ request, context: new RouterContextProvider() } as never) as typeof response;
    data = response.data;
    html = renderPage();
    expect(html).toContain('role="search"');
    expect(html).toContain('name="search"');
    expect(html).toContain("offset=144");
    expect(html).toContain("offset=96");
    expect(html).toContain("Page 6");
  });

  describe("and Today is selected", () => {
    beforeEach(() => { request = new Request("https://example.test/whats-on?filter=today"); });
    it("should keep cross-midnight live events while date-filtering upcoming events", async () => {
      response = await loader({ request, context: new RouterContextProvider() } as never) as typeof response;
      data = response.data;
      expect(vi.mocked(fetchEvents).mock.calls[0][0]).not.toHaveProperty("from");
      expect(Date.parse(vi.mocked(fetchEventPage).mock.calls[0][0]?.to ?? "") - Date.parse(vi.mocked(fetchEventPage).mock.calls[0][0]?.from ?? "")).toBe(86_400_000);
    });
  });

  describe("and the API is unavailable", () => {
    beforeEach(() => { vi.mocked(fetchEvents).mockRejectedValue(new Error("offline")); vi.mocked(fetchEventPage).mockRejectedValue(new Error("offline")); vi.mocked(fetchEventCalendar).mockRejectedValue(new Error("offline")); });
    it("should show a retryable failure instead of an empty catalogue", async () => {
      response = await loader({ request, context: new RouterContextProvider() } as never) as typeof response;
      data = response.data;
      html = renderPage();
      expect(html).toContain("Hangouts are temporarily unavailable");
      expect(html).toContain("Retry");
      expect(html).not.toContain("No events right now");
      expect(response.init?.status).toBe(503);
      expect(html).not.toContain("Next page");
    });
  });

  describe("and only the live feed fails", () => {
    beforeEach(() => {
      vi.mocked(fetchEvents).mockImplementation(async (params) => {
        if (params?.list === "live") throw new Error("offline");
        return { data: events, total: events.length };
      });
    });
    it("should keep the successful page and offer a retry for the missing feed", async () => {
      response = await loader({ request, context: new RouterContextProvider() } as never) as typeof response;
      data = response.data;
      expect(data.upcoming).toHaveLength(24);
      expect(renderPage()).toContain("Live hangouts are temporarily unavailable");
      expect(renderPage()).toContain("Next page");
      expect(response.init?.status).toBe(200);
    });
  });

  describe("and the calendar includes a later event outside the current page", () => {
    beforeEach(() => {
      vi.useFakeTimers();
      vi.setSystemTime(new Date("2099-06-01T12:00:00Z"));
      vi.mocked(fetchEventCalendar).mockResolvedValue({ data: [{ ...events[0], id: "later", start_at: "2099-06-20T18:00:00Z", next_start_at: "2099-06-20T18:00:00Z" }], total: 1 });
    });
    afterEach(() => { vi.useRealTimers(); });
    it("should keep its calendar entry independent of catalogue pagination", async () => {
      response = await loader({ request, context: new RouterContextProvider() } as never) as typeof response;
      data = response.data;
      expect(data.allDays[19].map((event) => event.id)).toContain("later");
      expect(fetchEventCalendar).toHaveBeenCalledWith(expect.objectContaining({ from: "2099-06-01T00:00:00.000Z", to: "2099-06-29T00:00:00.000Z" }), expect.anything());
      expect(vi.mocked(fetchEventCalendar).mock.calls[0][0]).not.toHaveProperty("offset");
    });
  });

  describe("and malformed data is the only problem", () => {
    beforeEach(() => { vi.mocked(fetchEventPage).mockResolvedValue({ data: events.slice(0, 24), hasMore: true, invalidCount: 1 }); });
    it("should show the data warning without offering a service retry", async () => {
      response = await loader({ request, context: new RouterContextProvider() } as never) as typeof response;
      data = response.data;
      html = renderPage();
      expect(html).toContain("Some hangouts could not be displayed");
      expect(html).not.toContain(">Retry<");
      expect(html).toContain("Next page");
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
