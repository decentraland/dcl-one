import { renderToString } from "react-dom/server";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fetchEvents, toLiveNowCard, type Event } from "@/data/lib/catalyst/places/events";
import WhatsOn, { loader } from "./whats-on";

vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => ({ identity: null, isConnected: false }) }));
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));
vi.mock("@/core/lib/telemetry/page-loader", () => ({ pageLoader: async () => ({ sid: "test-session", wrap: (data: unknown) => data }) }));
vi.mock("@/data/lib/catalyst/places/events", async (original) => ({ ...await original<typeof import("@/data/lib/catalyst/places/events")>(), fetchEvents: vi.fn() }));

describe("when the public hangout catalogue is paginated", () => {
  let events: Event[];
  let request: Request;
  let data: Awaited<ReturnType<typeof loader>>["data"];
  let html: string;
  let renderPage: () => string;
  beforeEach(() => {
    events = Array.from({ length: 25 }, (_, index) => ({ id: `event-${index}`, name: `Hangout ${index}`, start_at: "2099-06-01T18:00:00Z", next_start_at: "2099-06-01T18:00:00Z", recurrent: true, total_attendees: 90 } as Event));
    request = new Request("https://example.test/whats-on?search=music&offset=120&filter=recurring");
    vi.mocked(fetchEvents).mockImplementation(async (params) => ({ data: params?.list === "live" ? [] : events, total: events.length }));
    renderPage = () => {
      const router = createMemoryRouter([{ id: "feed", path: "/whats-on", Component: WhatsOn as never }], { initialEntries: [request.url.replace("https://example.test", "")], hydrationData: { loaderData: { feed: data } } });
      return renderToString(<RouterProvider router={router} />).replace(/<!--.*?-->/g, "");
    };
  });
  afterEach(() => { vi.restoreAllMocks(); vi.clearAllMocks(); });

  it("should request the selected page and apply recurrence and search before pagination", async () => {
    data = await loader({ request } as never) as unknown as typeof data;
    expect(fetchEvents).toHaveBeenCalledWith(expect.objectContaining({ list: "active", offset: 120, limit: 25, search: "music", recurrent: true }), expect.objectContaining({ signal: request.signal }));
    expect(fetchEvents).toHaveBeenCalledWith(expect.objectContaining({ list: "live", recurrent: true, with_connected_users: true }), expect.anything());
    expect(data.upcoming).toHaveLength(24);
    expect(data.hasMore).toBe(true);
  });

  it("should expose search and next/previous links retaining filters", async () => {
    data = await loader({ request } as never) as unknown as typeof data;
    html = renderPage();
    expect(html).toContain('role="search"');
    expect(html).toContain('name="search"');
    expect(html).toContain("offset=144");
    expect(html).toContain("offset=96");
    expect(html).toContain("Page 6");
  });

  describe("and Today is selected", () => {
    beforeEach(() => { request = new Request("https://example.test/whats-on?filter=today"); });
    it("should apply the same server date range to live and upcoming lists", async () => {
      data = await loader({ request } as never) as unknown as typeof data;
      expect(vi.mocked(fetchEvents).mock.calls[0][0]?.from).toEqual(vi.mocked(fetchEvents).mock.calls[1][0]?.from);
      expect(Date.parse(vi.mocked(fetchEvents).mock.calls[0][0]?.to ?? "") - Date.parse(vi.mocked(fetchEvents).mock.calls[0][0]?.from ?? "")).toBe(86_400_000);
    });
  });

  describe("and the API is unavailable", () => {
    beforeEach(() => { vi.mocked(fetchEvents).mockRejectedValue(new Error("offline")); });
    it("should show a retryable failure instead of an empty catalogue", async () => {
      data = await loader({ request } as never) as unknown as typeof data;
      html = renderPage();
      expect(html).toContain("Hangouts are temporarily unavailable");
      expect(html).toContain("Retry");
      expect(html).not.toContain("No events right now");
    });
  });

  describe("and a live event has many RSVPs", () => {
    it("should leave live occupancy unknown without a presence observation", () => {
      expect(toLiveNowCard(events[0]).users).toBeNull();
    });
    it("should count observed connected wallets independently of RSVPs", () => {
      expect(toLiveNowCard({ ...events[0], connected_addresses: ["0xAb", "0xab", "0xcd"] }).users).toBe(2);
    });
  });
});
