// @vitest-environment node
// Keep server route exports available before installing the DOM used for hydration.
import { builtinEnvironments } from "vitest/runtime";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, within, waitFor } from "@testing-library/react";
import { renderToString } from "react-dom/server";
import { hydrateRoot, type Root } from "react-dom/client";
import { type ComponentProps, type ReactNode } from "react";
import { createMemoryRouter, Outlet, RouterProvider, RouterContextProvider, useLoaderData } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { QUERY_CLIENT_CONFIG } from "@/core/lib/query/defaults";
import { sessionMiddleware } from "@/core/lib/telemetry/session";
import { destination } from "@/data/lib/catalyst/places/destination.fixture";
import { fetchDestinations } from "@/data/lib/catalyst/places/destinations";
import { loader as rootLoader, shouldRevalidate } from "../root";
import { track } from "@/core/lib/telemetry/track";
import { resetDappsFlags } from "@/data/lib/feature-flags.server";
import { primeRemoteFlags } from "@dcl/one-ui/apps/shop/remote-flags";
import Places, { loader, clientLoader, resetPlacesFlagsRead } from "./places";

vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => ({ isConnected: false, identity: null }) }));
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));
vi.mock("@/features/components/chrome/SitesLayout", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));
vi.mock("@/data/lib/catalyst/places/destinations", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/places/destinations")>()),
  fetchDestinations: vi.fn(),
}));

function Page() {
  return <Places {...({ loaderData: useLoaderData() } as ComponentProps<typeof Places>)} />;
}

let dom: Awaited<ReturnType<typeof builtinEnvironments.jsdom.setup>>;
beforeEach(async () => {
  vi.clearAllMocks();
  dom = await builtinEnvironments.jsdom.setup(globalThis, { jsdom: { url: "https://example.org/places" } });
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  resetDappsFlags();
  resetPlacesFlagsRead();
  primeRemoteFlags(null);
  serveFlags("0");
});

const FLAG = "dapps-places-live-min-user";

/**
 * The feature-flags service (as the server reads it) and this server's /internal/feature-flags, both with `variant`.
 * `serviceDown` makes the service fail, as on a cold start before its first answer.
 */
function serveFlags(variant: string, { serviceDown = false } = {}) {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string) => {
      if (String(url).endsWith("/dapps.json")) {
        if (serviceDown) return new Response("down", { status: 500 });
        const payload = { type: "string", value: variant };
        return Response.json({ flags: { [FLAG]: true }, variants: { [FLAG]: { name: "min_users", enabled: true, payload } } });
      }
      if (String(url).startsWith("/internal/feature-flags")) return Response.json({ flags: { [FLAG]: true }, variants: { [FLAG]: variant } });
      throw new Error(`unexpected fetch ${url}`);
    }),
  );
}
let hydrated: Root | undefined;
const clients: QueryClient[] = [];
const routers: ReturnType<typeof createMemoryRouter>[] = [];
afterEach(async () => {
  if (hydrated) await act(async () => hydrated?.unmount());
  hydrated = undefined;
  clients.splice(0).forEach((client) => client.clear());
  routers.splice(0).forEach((router) => router.dispose());
  await dom.teardown(globalThis);
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

async function documentData(url: string) {
  const request = new Request(url);
  const context = new RouterContextProvider();
  const args = { request, context, params: {}, url: new URL(url), pattern: "/places" };
  let root!: Awaited<ReturnType<typeof rootLoader>>;
  let places!: Awaited<ReturnType<typeof loader>>;
  await sessionMiddleware(args as Parameters<typeof sessionMiddleware>[0], async () => {
    [root, places] = await Promise.all([rootLoader(args as never), loader(args as never)]);
    return new Response();
  });
  return { root, places: places.data };
}

function tree(data: Awaited<ReturnType<typeof documentData>>) {
  const client = new QueryClient(QUERY_CLIENT_CONFIG);
  clients.push(client);
  const router = createMemoryRouter(
    [
      {
        id: "root",
        path: "/",
        Component: Outlet,
        loader: ({ request }) => rootLoader({ request, context: new RouterContextProvider(), params: {} } as never),
        shouldRevalidate,
        children: [{ id: "places", path: "places", Component: Page, loader: clientLoader }],
      },
    ],
    { initialEntries: ["/places"], hydrationData: { loaderData: { root: data.root, places: data.places } } },
  );
  routers.push(router);
  return {
    client,
    router,
    element: (
      <QueryClientProvider client={client}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    ),
  };
}

describe("when hydrating the real Places document", () => {
  it.each([true, false])("should preserve hydrated reads and markup with scrolling observation %s", async (observable) => {
    const rows = Array.from({ length: 48 }, (_, index) =>
      destination({ id: `scene-${index}`, title: `Scene ${index}`, contact_name: "SDK" }),
    );
    vi.mocked(fetchDestinations).mockImplementation(async (params) => ({
      data: params?.limit === 40 ? [] : rows,
      total: 96,
      received: params?.limit === 40 ? 0 : 48,
      copy: null,
    }));
    const data = await documentData("https://example.org/places");
    expect(data.root.sid).toBe(data.places.sid);
    expect(data.root.sid).toEqual(expect.any(String));
    const server = tree(data);
    const container = document.createElement("div");
    container.innerHTML = renderToString(server.element);
    expect(within(container).getByRole("button", { name: "Load more places" })).toBeTruthy();
    document.body.append(container);
    const observe = vi.fn();
    if (observable)
      vi.stubGlobal(
        "IntersectionObserver",
        class {
          observe = observe;
          disconnect = vi.fn();
        },
      );
    const recover = vi.fn();
    const browser = tree(data);
    await act(async () => {
      hydrated = hydrateRoot(container, browser.element, { onRecoverableError: recover });
    });
    if (observable) await waitFor(() => expect(observe).toHaveBeenCalled());
    expect(recover).not.toHaveBeenCalled();
    expect(Boolean(within(container).queryByRole("button", { name: "Load more places" }))).toBe(!observable);
    expect(fetchDestinations).toHaveBeenCalledTimes(2);
    expect(browser.client.getQueryData(["places", "browse", "all", ""])).toBeTruthy();
    expect(browser.client.getQueryData(["places", "live"])).toBeTruthy();
    if (!observable) {
      await act(async () => within(container).getByRole("button", { name: "Load more places" }).click());
      await waitFor(() => expect(fetchDestinations).toHaveBeenCalledWith(expect.objectContaining({ offset: 48 }), expect.anything()));
    }
    await act(async () => browser.router.navigate("/places?category=music"));
    await waitFor(() =>
      expect(track).toHaveBeenCalledWith("place_list_viewed", expect.objectContaining({ category: "music" }), {
        sid: data.root.sid,
        story: "misc/browse-places",
      }),
    );
    await act(async () => within(container).getByRole("link", { name: "Scene 0" }).click());
    expect(track).toHaveBeenCalledWith("place_card_clicked", { place_id: "scene-0" }, { sid: data.root.sid, story: "misc/browse-places" });
    expect(recover).not.toHaveBeenCalled();
  });

  it("should isolate interleaved document filters and session identities", async () => {
    const pending: {
      search?: string;
      resolve: (value: { data: ReturnType<typeof destination>[]; total: number; received: number; copy: null }) => void;
    }[] = [];
    vi.mocked(fetchDestinations).mockImplementation(
      (params) => new Promise((resolve) => pending.push({ search: params?.search, resolve })),
    );
    const first = documentData("https://example.org/places?search=alpha&category=music");
    const second = documentData("https://example.org/places?search=bravo&category=art");
    await waitFor(() => expect(pending).toHaveLength(2));
    for (const read of [...pending].reverse())
      read.resolve({ data: [destination({ id: read.search })], total: 1, received: 1, copy: null });
    const [a, b] = await Promise.all([first, second]);
    expect(a.root.sid).not.toBe(b.root.sid);
    expect(a.places.sid).toBe(a.root.sid);
    expect(b.places.sid).toBe(b.root.sid);
    expect(a.places.dehydratedState.queries[0]?.queryKey).toEqual(["places", "browse", "music", "alpha"]);
    expect(b.places.dehydratedState.queries[0]?.queryKey).toEqual(["places", "browse", "art", "bravo"]);
    expect(JSON.stringify(a.places.dehydratedState)).not.toContain('"bravo"');
    expect(JSON.stringify(b.places.dehydratedState)).not.toContain('"alpha"');
  });

  async function hydrateLive(url: string) {
    const live = [destination({ id: "busy", title: "Busy", user_count: 6 }), destination({ id: "quiet", title: "Quiet", user_count: 2 })];
    vi.mocked(fetchDestinations).mockImplementation(async (params) =>
      params?.limit === 40 ? { data: live, total: 2, received: 2, copy: null } : { data: [], total: 0, received: 0, copy: null },
    );
    const data = await documentData(url);
    const container = document.createElement("div");
    container.innerHTML = renderToString(tree(data).element);
    document.body.append(container);
    const browser = tree(data);
    await act(async () => {
      hydrated = hydrateRoot(container, browser.element);
    });
    const rail = () => within(container).getByRole("group", { name: "Live Now" });
    const flagReads = () => vi.mocked(fetch).mock.calls.filter(([called]) => String(called).startsWith("/internal/feature-flags")).length;
    return { data, browser, rail, flagReads };
  }

  describe("and the flag service had not answered the server yet", () => {
    it("should read the flags on the first client navigation and apply their head count", async () => {
      serveFlags("5", { serviceDown: true });
      const { data, browser, rail, flagReads } = await hydrateLive("https://example.org/places");
      expect(data.places.liveMinUsers).toBeNull();
      // Every scene while no head count is known.
      expect(within(rail()).getByRole("link", { name: "Quiet" })).toBeTruthy();
      await act(async () => browser.router.navigate("/places?category=music"));
      await waitFor(() => expect(browser.router.state.navigation.state).toBe("idle"));
      expect(flagReads()).toBe(1);
      await waitFor(() => expect(within(rail()).queryByRole("link", { name: "Quiet" })).toBeNull());
      expect(within(rail()).getByRole("link", { name: "Busy" })).toBeTruthy();
      // Known now: a later navigation does not read the flags again.
      await act(async () => browser.router.navigate("/places?category=art"));
      await waitFor(() => expect(browser.router.state.navigation.state).toBe("idle"));
      expect(flagReads()).toBe(1);
    });
  });

  describe("and the flag service answered a head count of 0", () => {
    it("should keep 0 on a client navigation without reading the flags", async () => {
      serveFlags("0");
      const { data, browser, rail, flagReads } = await hydrateLive("https://example.org/places");
      expect(data.places.liveMinUsers).toBe(0);
      serveFlags("5");
      await act(async () => browser.router.navigate("/places?category=music"));
      await waitFor(() => expect(browser.router.state.navigation.state).toBe("idle"));
      expect(within(rail()).getByRole("link", { name: "Quiet" })).toBeTruthy();
      expect(flagReads()).toBe(0);
    });
  });

  it("should hydrate Live Now under the flag's head count and keep it on a client navigation", async () => {
    serveFlags("5");
    const live = [destination({ id: "busy", title: "Busy", user_count: 6 }), destination({ id: "quiet", title: "Quiet", user_count: 2 })];
    vi.mocked(fetchDestinations).mockImplementation(async (params) =>
      params?.limit === 40 ? { data: live, total: 2, received: 2, copy: null } : { data: [], total: 0, received: 0, copy: null },
    );
    const data = await documentData("https://example.org/places");
    expect(data.places.liveMinUsers).toBe(5);
    const container = document.createElement("div");
    container.innerHTML = renderToString(tree(data).element);
    document.body.append(container);
    const rail = () => within(container).getByRole("group", { name: "Live Now" });
    // The server markup already leaves the quieter scene out.
    expect(within(rail()).getByRole("link", { name: "Busy" })).toBeTruthy();
    expect(within(rail()).queryByRole("link", { name: "Quiet" })).toBeNull();
    const recover = vi.fn();
    const browser = tree(data);
    await act(async () => {
      hydrated = hydrateRoot(container, browser.element, { onRecoverableError: recover });
    });
    expect(recover).not.toHaveBeenCalled();
    expect(within(rail()).getByRole("link", { name: "Busy" })).toBeTruthy();
    expect(within(rail()).queryByRole("link", { name: "Quiet" })).toBeNull();
    // This server's flags would now answer 0; the document keeps the head count it was rendered with.
    serveFlags("0");
    await act(async () => browser.router.navigate("/places?category=music"));
    await waitFor(() => expect(browser.router.state.navigation.state).toBe("idle"));
    expect(within(rail()).getByRole("link", { name: "Busy" })).toBeTruthy();
    expect(within(rail()).queryByRole("link", { name: "Quiet" })).toBeNull();
    expect(vi.mocked(fetch).mock.calls.filter(([url]) => String(url).startsWith("/internal/feature-flags"))).toEqual([]);
  });
});
