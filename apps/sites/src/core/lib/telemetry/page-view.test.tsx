// @vitest-environment jsdom
import { act, cleanup, render, waitFor } from "@testing-library/react";
import { StrictMode, useSyncExternalStore } from "react";
import { createMemoryRouter, Outlet, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

const page = vi.hoisted(() => vi.fn());
vi.mock("./track", () => ({ page, adoptSitesSegmentConfig: vi.fn() }));

import { dropTrackedUser, restoreTrackedUser } from "./tracked-user";
import { isOutsideLegacySites, isPageTrackingExempt, lookupSession, useNotFoundPageView, usePageView, useRoutePageView, type PageView } from "./page-view";

// A view a test sets once its "data" arrives, as a page reading it from a query does.
let pendingView: PageView | null = null;
const listeners = new Set<() => void>();
function setPendingView(view: PageView | null) {
  pendingView = view;
  listeners.forEach((l) => l());
}
function usePendingView() {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => pendingView,
  );
}

function Root() {
  useRoutePageView();
  return <Outlet />;
}
function BlogHome() {
  usePageView({ name: "Blog", properties: { title: "Blog", path: "/blog" } });
  return null;
}
function LoadingCommunity() {
  usePageView(usePendingView());
  return null;
}

function setup(session: { sid?: string | null; wallet?: string | null; analyticsUserId?: string | null } = { sid: "sid-1" }, initial = "/", strict = false) {
  const router = createMemoryRouter(
    [
      {
        id: "root",
        path: "/",
        loader: () => session,
        Component: Root,
        HydrateFallback: () => null,
        children: [
          { index: true, Component: () => null },
          { path: "blog", Component: BlogHome },
          { path: "social/communities/:id", Component: LoadingCommunity },
          { path: "*", Component: () => null },
        ],
      },
    ],
    { initialEntries: [initial] },
  );
  const tree = <RouterProvider router={router} />;
  render(strict ? <StrictMode>{tree}</StrictMode> : tree);
  return router;
}

const names = () => page.mock.calls.map(([name]) => name);

describe("usePageView", () => {
  beforeEach(() => {
    page.mockReset();
    setPendingView(null);
  });
  afterEach(cleanup);

  it("sends page(pathname) on the first load with the session id and the verified wallet as userId", async () => {
    setup({ sid: "sid-1", analyticsUserId: "0xabc" }, "/create");
    await waitFor(() => expect(page).toHaveBeenCalledTimes(1));
    expect(page).toHaveBeenCalledWith("/create", {}, { sid: "sid-1", userId: "0xabc" });
  });

  it("never sends the display wallet as userId", async () => {
    setup({ sid: "sid-1", wallet: "0xdisplay", analyticsUserId: null }, "/create");
    await waitFor(() => expect(page).toHaveBeenCalledTimes(1));
    expect(page.mock.calls[0]?.[2]).toEqual({ sid: "sid-1" });
  });

  it("sends again on a pathname change, not on a search-only change", async () => {
    const router = setup(undefined, "/create");
    await waitFor(() => expect(page).toHaveBeenCalledTimes(1));
    await act(() => router.navigate("/create?utm_source=x"));
    await act(() => router.navigate("/events"));
    await waitFor(() => expect(names()).toEqual(["/create", "/events"]));
    await act(() => router.navigate("/events?tab=live"));
    await act(() => router.navigate("/create"));
    await waitFor(() => expect(names()).toEqual(["/create", "/events", "/create"]));
  });

  it("sends once per navigation under StrictMode", async () => {
    const router = setup(undefined, "/help", true);
    await waitFor(() => expect(page).toHaveBeenCalledTimes(1));
    await act(() => router.navigate("/invite/someone"));
    await waitFor(() => expect(names()).toEqual(["/help", "/invite/someone"]));
  });

  it("sends nothing on the suppressed paths, and does on /events", async () => {
    const router = setup(undefined, "/whats-on");
    // Routes match case-insensitively, so a case variant is the same page and its own view.
    for (const path of ["/discover/places", "/events/event", "/cast", "/storage/env", "/places/abc", "/social/communities/1", "/Blog/news/post", "/PLACES/abc"]) {
      await act(() => router.navigate(path));
    }
    await act(() => router.navigate("/events"));
    await waitFor(() => expect(names()).toEqual(["/events"]));
  });

  it("sends nothing on surfaces the legacy site never served, the Shop included", async () => {
    const router = setup(undefined, "/shop/items");
    for (const path of ["/marketplace/browse", "/builder/collections", "/play", "/auth/callback", "/creator-hub/x", "/SHOP/items", "/docs/sdk/intro"]) {
      await act(() => router.navigate(path));
    }
    await act(() => router.navigate("/create"));
    await waitFor(() => expect(names()).toEqual(["/create"]));
  });

  it("sends a page again after leaving through a suppressed path and coming back", async () => {
    const router = setup(undefined, "/events");
    await waitFor(() => expect(page).toHaveBeenCalledTimes(1));
    await act(() => router.navigate("/whats-on"));
    await act(() => router.navigate("/events"));
    await waitFor(() => expect(names()).toEqual(["/events", "/events"]));
  });

  it("sends a page again after leaving through an excluded surface and coming back", async () => {
    const router = setup(undefined, "/events");
    await waitFor(() => expect(page).toHaveBeenCalledTimes(1));
    await act(() => router.navigate("/shop/items"));
    await act(() => router.navigate("/events"));
    await waitFor(() => expect(names()).toEqual(["/events", "/events"]));
  });

  it("sends a page again after a named page in between", async () => {
    const router = setup(undefined, "/events", true);
    await waitFor(() => expect(page).toHaveBeenCalledTimes(1));
    await act(() => router.navigate("/blog"));
    await act(() => router.navigate("/events?tab=live"));
    await waitFor(() => expect(names()).toEqual(["/events", "Blog", "/events"]));
  });

  it("lets a route's own page view replace the default for that navigation", async () => {
    const router = setup(undefined, "/create");
    await waitFor(() => expect(page).toHaveBeenCalledTimes(1));
    await act(() => router.navigate("/blog"));
    await waitFor(() => expect(page).toHaveBeenCalledTimes(2));
    expect(page.mock.calls[1]).toEqual(["Blog", { title: "Blog", path: "/blog" }, { sid: "sid-1" }]);
    await act(() => router.navigate("/blog?page=2"));
    expect(page).toHaveBeenCalledTimes(2);
  });

  it("waits for a named page's data, then sends it once, and again when its properties change", async () => {
    setup(undefined, "/social/communities/c1");
    await act(async () => {});
    expect(page).not.toHaveBeenCalled();
    act(() => setPendingView({ name: "Builders", properties: { communityId: "c1", privacy: "public", membersCount: 3 } }));
    await waitFor(() => expect(page).toHaveBeenCalledTimes(1));
    expect(page.mock.calls[0]?.[0]).toBe("Builders");
    act(() => setPendingView({ name: "Builders", properties: { communityId: "c1", privacy: "public", membersCount: 3 } }));
    expect(page).toHaveBeenCalledTimes(1);
    act(() => setPendingView({ name: "Builders", properties: { communityId: "c1", privacy: "public", membersCount: 4 } }));
    await waitFor(() => expect(page).toHaveBeenCalledTimes(2));
  });

  it("sends a property whose value is undefined as the view had it", async () => {
    setup(undefined, "/social/communities/c1");
    act(() => setPendingView({ name: "Builders", properties: { communityId: "c1", thumbnail: undefined } }));
    await waitFor(() => expect(page).toHaveBeenCalledTimes(1));
    const properties = page.mock.calls[0]?.[1] as Record<string, unknown>;
    expect(Object.keys(properties)).toEqual(["communityId", "thumbnail"]);
  });

  it("sends nothing without a session id", async () => {
    const router = setup({ sid: null }, "/create");
    await act(() => router.navigate("/blog"));
    await act(async () => {});
    expect(page).not.toHaveBeenCalled();
  });

});

describe("useNotFoundPageView", () => {
  let fetchMock: ReturnType<typeof vi.fn>;
  beforeEach(() => {
    page.mockReset();
    fetchMock = vi.fn(async () => Response.json({ sid: "fetched-sid", userId: "0xverified" }));
    vi.stubGlobal("fetch", fetchMock);
  });
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  function NotFoundAt({ notFound }: { notFound: boolean }) {
    useNotFoundPageView(notFound);
    return null;
  }

  // `root` null: the cold 404 of a URL no route matches, rendered without the root loader's data.
  function renderAt(path: string, notFound: boolean, root: { sid?: string | null } | null = { sid: "sid-1" }, strict = false) {
    const router = createMemoryRouter(
      [{ id: "root", path: "*", loader: () => root, Component: () => <NotFoundAt notFound={notFound} />, HydrateFallback: () => null }],
      { initialEntries: [path] },
    );
    const tree = <RouterProvider router={router} />;
    render(strict ? <StrictMode>{tree}</StrictMode> : tree);
  }

  it("looks the session up once under StrictMode, and sends with the sid that one response set", async () => {
    renderAt("/missing/strict", true, null, true);
    await waitFor(() => expect(page).toHaveBeenCalledTimes(1));
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(page.mock.calls[0]?.[2]).toEqual({ sid: "fetched-sid", userId: "0xverified" });
  });

  it("shares one lookup between concurrent 404 views, then looks up afresh once it settled", async () => {
    let answer: (response: Response) => void = () => {};
    fetchMock.mockImplementationOnce(() => new Promise<Response>((resolve) => { answer = resolve; }));
    const first = lookupSession();
    const second = lookupSession();
    expect(second).toBe(first);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    answer(Response.json({ sid: "s-1", userId: "0xold" }));
    expect(await first).toEqual({ sid: "s-1", userId: "0xold" });
    fetchMock.mockResolvedValueOnce(Response.json({ sid: "s-1", userId: null }));
    expect(await lookupSession()).toEqual({ sid: "s-1", userId: "" });
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it("looks up afresh after a failed lookup", async () => {
    fetchMock.mockRejectedValueOnce(new Error("offline"));
    expect(await lookupSession()).toBeNull();
    expect(await lookupSession()).toEqual({ sid: "fetched-sid", userId: "0xverified" });
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  // The root error boundary renders it in place of the app, so the default page(pathname) is not sent with it.
  it("sends the 404's own page view with the root session, without fetching one", async () => {
    renderAt("/missing/thing", true);
    await waitFor(() => expect(page).toHaveBeenCalledTimes(1));
    expect(page).toHaveBeenCalledWith("Not Found", { path: "/missing/thing" }, { sid: "sid-1" });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("sends a cold 404's page view with the session it fetches", async () => {
    renderAt("/missing/deep/thing", true, null);
    await waitFor(() => expect(page).toHaveBeenCalledTimes(1));
    expect(fetchMock).toHaveBeenCalledWith("/internal/session", expect.objectContaining({ cache: "no-store" }));
    expect(page).toHaveBeenCalledWith("Not Found", { path: "/missing/deep/thing" }, { sid: "fetched-sid", userId: "0xverified" });
  });

  it("sends a cold 404 anonymously while a sign-out is in progress, even when the fetched session still names the wallet", async () => {
    dropTrackedUser();
    try {
      fetchMock.mockResolvedValueOnce(Response.json({ sid: "fetched-sid", userId: "0xold" }));
      renderAt("/missing/after/signout", true, null, true);
      await waitFor(() => expect(page).toHaveBeenCalledTimes(1));
      await act(async () => {});
      expect(page).toHaveBeenCalledTimes(1);
      expect(page.mock.calls[0]?.[2]).toEqual({ sid: "fetched-sid" });
    } finally {
      restoreTrackedUser();
    }
  });

  it("sends nothing, and does not throw, when the session cannot be fetched", async () => {
    fetchMock.mockRejectedValueOnce(new Error("offline"));
    renderAt("/missing/a/b", true, null);
    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
    await act(async () => {});
    expect(page).not.toHaveBeenCalled();
  });

  it("sends nothing when the session endpoint answers an error or no sid", async () => {
    const failed = new Response("nope", { status: 500 });
    const cancel = vi.spyOn(failed.body!, "cancel");
    fetchMock.mockResolvedValueOnce(failed);
    renderAt("/missing/a/b", true, null);
    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
    // The error's body is released, not left unread.
    await waitFor(() => expect(cancel).toHaveBeenCalledOnce());
    cleanup();
    fetchMock.mockResolvedValueOnce(Response.json({}));
    renderAt("/missing/a/c", true, null);
    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(2));
    await act(async () => {});
    expect(page).not.toHaveBeenCalled();
  });

  it.each([
    ["with the root session", { sid: "sid-1" }],
    ["on a cold load", null],
  ])("sends nothing for a missing Shop page %s", async (_label, root) => {
    renderAt("/shop/unknown/item", true, root);
    await act(async () => {});
    expect(page).not.toHaveBeenCalled();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("sends nothing for another error status", async () => {
    renderAt("/nope", false);
    await act(async () => {});
    expect(page).not.toHaveBeenCalled();
  });

  it("sends nothing for a missing page under a path whose pages own their view", async () => {
    renderAt("/places/unknown/deep", true, null);
    await act(async () => {});
    expect(page).not.toHaveBeenCalled();
    expect(fetchMock).not.toHaveBeenCalled();
  });
});

describe("isOutsideLegacySites", () => {
  it.each(["/shop", "/shop/items", "/shop-native", "/marketplace", "/builder/x", "/play", "/auth/callback", "/internal/client-error", "/api/x", "/docs", "/docs/sdk/intro", "/SHOP", "/Shop/items", "/Docs/x"])(
    "excludes %s",
    (path) => expect(isOutsideLegacySites(path)).toBe(true),
  );

  it.each(["/", "/shopping", "/create", "/help", "/download", "/events", "/players", "/docsx", "/communities/new"])(
    "keeps %s",
    (path) => expect(isOutsideLegacySites(path)).toBe(false),
  );
});

describe("isPageTrackingExempt", () => {
  it.each(["/blog", "/blog/news", "/whats-on", "/whats-on/1", "/discover", "/discover/x", "/events/event", "/cast", "/cast/s/k", "/storage", "/storage/env", "/social", "/social/communities/1", "/places", "/places/world/x", "/Blog/x/y", "/BLOG", "/Places/abc", "/SOCIAL"])(
    "suppresses %s",
    (path) => expect(isPageTrackingExempt(path)).toBe(true),
  );

  it.each(["/", "/events", "/events/event/1", "/blogs", "/placesx", "/create", "/download", "/account/confirm-email/abc"])(
    "keeps %s",
    (path) => expect(isPageTrackingExempt(path)).toBe(false),
  );
});
