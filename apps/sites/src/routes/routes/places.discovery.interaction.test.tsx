// @vitest-environment jsdom
import { useSyncExternalStore } from "react";
import { useAuth } from "@/data/lib/auth/context";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AuthIdentity } from "@/data/lib/auth/types";
import type { Place } from "@/data/lib/catalyst/schema";
import { fetchDestinations, type DestinationPage } from "@/data/lib/catalyst/places/destinations";
import Places, { clientLoader, shouldRevalidate } from "./places";

const auth = vi.hoisted(() => ({ identity: null as AuthIdentity | null, listeners: new Set<() => void>() }));
vi.hoisted(() => { Object.assign(window, { __vite_plugin_react_preamble_installed__: true, $RefreshSig$: () => (type: unknown) => type }); });
vi.mock("@/data/lib/auth/context", () => ({ useAuth: vi.fn() }));
vi.mock("@ui/explorer/frames/ExploreChrome", () => ({ default: ({ children }: { children: React.ReactNode }) => <>{children}</>, EXPLORE_TABS: [], PUBLIC_WORLD_TABS: new Set() }));
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));
vi.mock("@/core/lib/telemetry/page-loader", () => ({ pageLoader: async () => ({ sid: "test", wrap: (data: unknown) => data }) }));
vi.mock("@/data/lib/catalyst/places/destinations", async (original) => ({ ...await original<typeof import("@/data/lib/catalyst/places/destinations")>(), fetchDestinations: vi.fn() }));

describe("when interacting with Places discovery", () => {
  let identity: AuthIdentity;
  let page: DestinationPage;
  let second: DestinationPage;
  let resolveNext: (page: DestinationPage) => void;
  let user: ReturnType<typeof userEvent.setup>;
  let router: ReturnType<typeof createMemoryRouter>;
  let routeLoader: (args: { request: Request }) => Promise<unknown>;
  let open: (url: string) => Promise<void>;
  let changeIdentity: (next: AuthIdentity | null) => void;
  beforeEach(() => {
    identity = { signer: "0x1111111111111111111111111111111111111111", ephemeral: { address: "0x2222222222222222222222222222222222222222", privateKey: "0x01" }, expiration: "2099-01-01T00:00:00Z", authChain: [] };
    page = { data: Array.from({ length: 60 }, (_, index): Place => ({ id: `place-${index}`, title: `Place ${index}`, description: null, image: "https://example.test/cover.png", owner: identity.signer, positions: ["1,2"], base_position: "1,2", updated_at: null, created_at: null, contact_name: "Creator", categories: [], highlighted: false, highlighted_image: null, user_count: 0, user_visits: 0, favorites: 0, likes: 0, like_rate: null, world: false, world_name: null })), total: 61, copy: null };
    second = { data: [{ ...page.data[0]!, id: "last", title: "Last place" }], total: 61, copy: null };
    auth.identity = identity;
    vi.mocked(useAuth).mockImplementation(() => {
      const current = useSyncExternalStore((listener) => { auth.listeners.add(listener); return () => { auth.listeners.delete(listener); }; }, () => auth.identity, () => null);
      return { identity: current, isConnected: Boolean(current), address: current?.signer ?? null } as ReturnType<typeof useAuth>;
    });
    user = userEvent.setup();
    vi.mocked(fetchDestinations).mockResolvedValue(page);
    routeLoader = vi.fn(async ({ request }: { request: Request }) => {
      const url = new URL(request.url);
      const section = url.searchParams.get("tab") ?? "all";
      const offset = Number(url.searchParams.get("offset") ?? 0);
      let result: DestinationPage = { data: [], total: 0, copy: null };
      let unavailable = false;
      if (section === "all") {
        try { result = await fetchDestinations({ offset }); } catch { unavailable = true; }
      }
      return { sid: "test", category: "", search: url.searchParams.get("search") ?? "", section, offset, places: result.data, total: result.total, received: result.received, categories: [], live: section === "all" && offset === 0 ? result.data.filter((place) => (place.user_count ?? 0) > 0) : [], featured: [], unavailable, layoutV2: false, copy: { stamp: null, now: 0 } };
    });
    open = async (url) => {
      // The route cache outlives router instances; each interaction starts with a fresh page.
      shouldRevalidate({ currentUrl: new URL(url, "https://example.test"), nextUrl: new URL(url, "https://example.test"), defaultShouldRevalidate: true } as never);
      router = createMemoryRouter([{ path: "/places", loader: ({ request }) => clientLoader({ request, serverLoader: () => routeLoader({ request }) } as never), shouldRevalidate, Component: Places as never }], { initialEntries: [url] });
      render(<RouterProvider router={router} />);
      await screen.findByRole("heading", { name: "Places", level: 1 });
    };
    changeIdentity = (next) => { act(() => { auth.identity = next; auth.listeners.forEach((listener) => listener()); }); };
  });
  afterEach(() => { cleanup(); router?.dispose(); vi.resetAllMocks(); auth.listeners.clear(); });

  describe("and the public feed is unavailable", () => {
    beforeEach(() => { vi.mocked(fetchDestinations).mockRejectedValue(new Error("offline")); });
    it("should refresh through the router and client loader on Retry and replace the error with fresh places", async () => {
      await open("/places?search=retry");
      await screen.findByRole("alert");
      vi.mocked(fetchDestinations).mockResolvedValueOnce(second);
      await user.click(screen.getByRole("button", { name: "Retry" }));
      await screen.findByRole("link", { name: /Last place/ });
      expect(routeLoader).toHaveBeenCalledTimes(2);
      expect(screen.queryByRole("alert")).toBeNull();
    });
  });

  describe("and favourites have another page", () => {
    beforeEach(() => {
      vi.mocked(fetchDestinations).mockResolvedValueOnce(page).mockImplementationOnce(() => new Promise((resolve) => { resolveNext = resolve; }));
    });
    it("should retain rows and a disabled loading button without calling the server loader again", async () => {
      await open("/places?tab=favourites");
      await user.click(await screen.findByRole("button", { name: "Load more places" }));
      expect((screen.getByRole("button", { name: "Loading…" }) as HTMLButtonElement).disabled).toBe(true);
      expect(screen.getByRole("link", { name: /Place 0Creator/ })).toBeTruthy();
      await act(async () => resolveNext(second));
      await screen.findByRole("link", { name: /Last place/ });
      expect(screen.getAllByRole("link")).toHaveLength(61);
      expect(routeLoader).toHaveBeenCalledTimes(1);
      expect(fetchDestinations).toHaveBeenLastCalledWith(expect.objectContaining({ offset: 60, only_favorites: true }), expect.objectContaining({ identity }));
    });
  });

  describe("and the next personal page fails", () => {
    beforeEach(() => { vi.mocked(fetchDestinations).mockResolvedValueOnce(page).mockRejectedValueOnce(new Error("Please retry your places")); });
    it("should keep loaded rows through an error and retry only the failed offset", async () => {
      await open("/places?tab=my");
      await user.click(await screen.findByRole("button", { name: "Load more places" }));
      await screen.findByRole("alert");
      expect(screen.getAllByRole("link")).toHaveLength(60);
      vi.mocked(fetchDestinations).mockResolvedValueOnce(second);
      await user.click(screen.getByRole("button", { name: "Retry" }));
      await screen.findByRole("link", { name: /Last place/ });
      expect(screen.getAllByRole("link")).toHaveLength(61);
      expect(fetchDestinations).toHaveBeenLastCalledWith(expect.objectContaining({ offset: 60, owner: identity.signer }), expect.objectContaining({ identity }));
    });
  });

  it("should preserve the initial signed-in deep link and reset pagination on account changes", async () => {
    await open("/places?tab=favourites&offset=60");
    await screen.findByRole("link", { name: /Place 0Creator/ });
    expect(router.state.location.search).toContain("offset=60");
    vi.mocked(fetchDestinations).mockImplementationOnce(() => new Promise((resolve) => { resolveNext = resolve; }));
    changeIdentity({ ...identity, signer: "0x3333333333333333333333333333333333333333" });
    expect(screen.queryByRole("link", { name: /Place 0Creator/ })).toBeNull();
    await waitFor(() => expect(router.state.location.search).not.toContain("offset"));
    expect(fetchDestinations).toHaveBeenCalledTimes(2);
    expect(fetchDestinations).toHaveBeenLastCalledWith(expect.objectContaining({ offset: 0 }), expect.objectContaining({ identity: expect.objectContaining({ signer: "0x3333333333333333333333333333333333333333" }) }));
    await act(async () => resolveNext(second));
    await screen.findByRole("link", { name: /Last place/ });
    changeIdentity(null);
    expect(screen.queryByRole("link", { name: /Last place/ })).toBeNull();
    expect(screen.getByRole("button", { name: "Sign in" })).toBeTruthy();
  });

  describe("and identity hydrates after the page mounts", () => {
    beforeEach(() => { auth.identity = null; });
    it("should read the signed-in deep-link offset without resetting it", async () => {
      await open("/places?tab=favourites&offset=60");
      await screen.findByRole("button", { name: "Sign in" });
      changeIdentity(identity);
      await screen.findByRole("link", { name: /Place 0Creator/ });
      expect(router.state.location.search).toContain("offset=60");
      expect(fetchDestinations).toHaveBeenCalledOnce();
      expect(fetchDestinations).toHaveBeenCalledWith(expect.objectContaining({ offset: 60 }), expect.objectContaining({ identity }));
    });
  });

  describe("and a later public page has different activity leaders", () => {
    beforeEach(() => {
      page.data[0]!.user_count = 8;
      second.data[0]!.user_count = 99;
      vi.mocked(fetchDestinations).mockResolvedValueOnce(page).mockResolvedValueOnce(second);
    });
    it("should retain the original live rail while appending later results once", async () => {
      await open("/places");
      await user.click(screen.getByRole("button", { name: "Load more places" }));
      await screen.findByRole("link", { name: /Last place/ });
      expect(screen.getByRole("region", { name: "Live places" }).textContent).toContain("Place 0");
      expect(screen.getByRole("region", { name: "Live places" }).textContent).not.toContain("Last place");
      expect(screen.getAllByRole("link")).toHaveLength(61);
    });
  });

  describe("and schema drift removed a row from a full page", () => {
    beforeEach(() => { page.data.pop(); page.received = 60; });
    it("should continue at the raw page boundary", async () => {
      await open("/places?search=drift");
      await user.click(screen.getByRole("button", { name: "Load more places" }));
      await waitFor(() => expect(router.state.location.search).toContain("offset=60"));
    });
  });

  describe("and all rows on a public page are hidden", () => {
    beforeEach(() => { page.data = page.data.map((place) => ({ ...place, title: "Empty" })); });
    it("should offer continuation without claiming that the catalogue is empty", async () => {
      await open("/places");
      expect(screen.getByText(/No visible places on this page/)).toBeTruthy();
      expect(screen.getByRole("button", { name: "Load more places" })).toBeTruthy();
      expect(screen.queryByText("No places found in this view.")).toBeNull();
    });
  });
});

describe("when the Places client loader chooses its cache", () => {
  let serverLoader: ReturnType<typeof vi.fn>;
  let request: Request;
  beforeEach(() => { serverLoader = vi.fn().mockResolvedValue({ unavailable: false }); });
  afterEach(() => vi.resetAllMocks());
  describe.each(["my", "favourites", "favorites"])("and the tab is %s", (tab) => {
    beforeEach(() => { request = new Request(`https://example.test/places?tab=${tab}`); });
    it("should always bypass cached personal shells", async () => {
      await clientLoader({ request, serverLoader } as never);
      await clientLoader({ request, serverLoader } as never);
      expect(serverLoader).toHaveBeenCalledTimes(2);
    });
  });
  describe("and a cached public page is manually refreshed", () => {
    beforeEach(() => { request = new Request("https://example.test/places?search=cache-refresh"); });
    it("should evict the old page before requesting fresh data", async () => {
      await clientLoader({ request, serverLoader } as never);
      await clientLoader({ request, serverLoader } as never);
      expect(serverLoader).toHaveBeenCalledTimes(1);
      expect(shouldRevalidate({ currentUrl: new URL(request.url), nextUrl: new URL(request.url), defaultShouldRevalidate: true } as never)).toBe(true);
      await clientLoader({ request, serverLoader } as never);
      expect(serverLoader).toHaveBeenCalledTimes(2);
    });
  });
});
