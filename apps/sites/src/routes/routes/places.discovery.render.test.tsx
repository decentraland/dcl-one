import { renderToString } from "react-dom/server";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Place } from "@/data/lib/catalyst/schema";
import { fetchDestinations } from "@/data/lib/catalyst/places/destinations";
import Places, { loader } from "./places";

vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => ({ identity: null, address: null, isConnected: false }) }));
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));
vi.mock("@/core/lib/telemetry/page-loader", () => ({ pageLoader: async () => ({ sid: "test-session", wrap: (data: unknown) => data }) }));
vi.mock("@/data/lib/catalyst/places/index.server", () => ({ loadCategories: async () => [] }));
vi.mock("@/data/lib/catalyst/places/destinations", async (original) => ({ ...await original<typeof import("@/data/lib/catalyst/places/destinations")>(), fetchDestinations: vi.fn() }));

describe("when the actual Places page renders unified discovery", () => {
  let busy: Place;
  let featured: Place;
  let regular: Place;
  let data: { sid: string; category: string; search: string; section: "all" | "favourites" | "my"; offset: number; places: Place[]; total: number; categories: []; live: Place[]; featured: Place[]; unavailable: boolean; layoutV2: boolean; copy: { stamp: null; now: number } };
  let renderPage: () => string;
  let html: string;

  beforeEach(() => {
    busy = { id: "busy", title: "Busy scene", description: null, image: "https://example.com/cover.png", owner: "0x1", positions: ["1,2"], base_position: "1,2", updated_at: null, created_at: null, contact_name: "Creator", categories: ["music"], highlighted: false, highlighted_image: null, user_count: 8, user_visits: 0, favorites: 0, likes: 0, like_rate: null, world: false, world_name: null };
    featured = { ...busy, id: "featured", title: "Featured world", highlighted: true, user_count: 0, world: true, world_name: "featured.dcl.eth" };
    regular = { ...busy, id: "regular", title: "Another scene", user_count: 0 };
    data = { sid: "test-session", category: "", search: "", section: "all", offset: 0, places: [busy, featured, regular], total: 3, categories: [], live: [busy, featured, regular], featured: [featured], unavailable: false, layoutV2: false, copy: { stamp: null, now: 0 } };
    vi.mocked(fetchDestinations).mockResolvedValue({ data: [], total: 0, copy: null });
    renderPage = () => {
      const router = createMemoryRouter([{ id: "places", path: "/places", Component: Places as never }], { initialEntries: ["/places"], hydrationData: { loaderData: { places: data } } });
      return renderToString(<RouterProvider router={router} />).replace(/<!--.*?-->/g, "");
    };
  });
  afterEach(() => vi.resetAllMocks());

  it("should render live and featured rails without repeating their cards in Explore all", () => {
    html = renderPage();
    expect(html).toContain("Live Now");
    expect(html).toContain("Featured Places");
    expect(html.match(/href="\/places\/busy"/g)).toHaveLength(1);
    expect(html.match(/href="\/places\/featured"/g)).toHaveLength(1);
  });

  it("should provide reachable favourites and personal places tabs", () => {
    html = renderPage();
    expect(html).toContain("Favourites");
    expect(html).toContain("My places");
    expect(html).toContain("featured.dcl.eth");
  });

  it("should read the live rail using activity order on the server", async () => {
    await loader({ request: new Request("https://example.test/places"), params: {}, context: {} } as never);
    expect(fetchDestinations).toHaveBeenCalledWith({ limit: 40, order_by: "most_active" }, expect.objectContaining({ signal: expect.any(AbortSignal) }));
  });

  it("should defer personal reads until the browser has the signed identity", async () => {
    await loader({ request: new Request("https://example.test/places?tab=favourites"), params: {}, context: {} } as never);
    expect(fetchDestinations).not.toHaveBeenCalled();
  });

  describe("and the favourites tab is signed out", () => {
    beforeEach(() => { data.section = "favourites"; });
    it("should show a sign-in action without rendering public rows as favourites", () => {
      html = renderPage();
      expect(html).toContain("Sign in to see your favourites");
      expect(html).not.toContain('href="/places/busy"');
    });
  });

  describe("and a category explicitly matches an unfinished scene", () => {
    beforeEach(() => { data.category = "music"; data.places = [{ ...regular, image: null, title: "Filtered unfinished scene" }]; data.total = 1; });
    it("should keep the explicit category match and collapse curated rails", () => {
      html = renderPage();
      expect(html).toContain("Filtered unfinished scene");
      expect(html).not.toContain("Featured Places");
    });
  });

  describe("and the catalogue is unavailable", () => {
    beforeEach(() => { data.unavailable = true; data.places = []; data.live = []; data.featured = []; });
    it("should render a retryable service error instead of an empty-catalogue claim", () => {
      html = renderPage();
      expect(html).toContain('role="alert"');
      expect(html).toContain("Places are temporarily unavailable");
      expect(html).not.toContain("No places found");
    });
  });
});
