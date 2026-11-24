import { createMemoryRouter, RouterContextProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { isPublicResponsePath, markPublicResponse } from "@/core/lib/telemetry/session";
import { postAuthCallback, resetSessionChangeForTests, sessionChangeApplied, sessionChangePending } from "@/data/lib/auth/session-change";

import { loader, shouldRevalidate } from "./root";

// The page view reads `sid` and `analyticsUserId` from the root route's data (core/lib/telemetry/
// page-view.test.tsx covers that), so what decides the userId after a sign-in or sign-out is whether
// the root loader runs again. This drives root.tsx's real loader and real shouldRevalidate through a
// data router, with the sign-in and sign-out posts made through the app's own `postAuthCallback`.
// (The loader runs here, in Node: the client build of a route module, which a DOM test environment
// loads, has its loader stripped.)

const WALLET = "0x4444444444444444444444444444444444444444";

// The browser's cookies as the server sees them on the next request: `/auth/callback` sets or clears
// `dcl_wallet_verified`, and the visitor then navigates.
let cookies = "";
const rootLoader = vi.fn(({ request }: { request: Request }) => {
  const context = new RouterContextProvider();
  // What routes/docs.$.tsx's middleware does before root's loader runs on a docs request.
  if (isPublicResponsePath(new URL(request.url).pathname)) markPublicResponse(context);
  return loader({ request: new Request(request.url, { headers: { cookie: cookies } }), params: {}, context } as never);
});

async function start(initial: string) {
  const router = createMemoryRouter(
    [{ id: "root", path: "/", loader: rootLoader, shouldRevalidate, children: [{ path: "*", loader: () => null }] }],
    { initialEntries: [initial] },
  );
  await vi.waitFor(() => expect(router.state.initialized).toBe(true));
  // What useSessionChangeRevalidation does in the app: root's data reloading applies the change.
  let seen = router.state.loaderData.root;
  router.subscribe((state) => {
    if (state.loaderData.root !== seen) {
      seen = state.loaderData.root;
      sessionChangeApplied();
    }
  });
  return router;
}

const rootData = (router: Awaited<ReturnType<typeof start>>) =>
  router.state.loaderData.root as { sid: string | null; analyticsUserId: string | null };

/** The server's answer to the post: it sets or clears the verified cookie the next request sends. */
function answerCallback(nextCookies: string) {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => {
      cookies = nextCookies;
      return Response.json({ ok: true });
    }),
  );
}

describe("the root data the page view reads, across sign-in and sign-out", () => {
  beforeEach(() => {
    rootLoader.mockClear();
    cookies = "sid=visitor-sid";
    // No change left over from another test.
    resetSessionChangeForTests();
  });
  afterEach(() => vi.unstubAllGlobals());

  it("names the new account after signing in, on the next navigation to another page", async () => {
    const router = await start("/events");
    expect(rootData(router)).toMatchObject({ sid: "visitor-sid", analyticsUserId: null });

    answerCallback(`sid=visitor-sid; dcl_wallet_verified=${WALLET}`);
    await postAuthCallback({ token: "t" });
    await router.navigate("/help");
    expect(rootLoader).toHaveBeenCalledTimes(2);
    expect(rootData(router)).toMatchObject({ sid: "visitor-sid", analyticsUserId: WALLET });
  });

  it("names no account after signing out, on the next navigation to another page", async () => {
    cookies = `sid=visitor-sid; dcl_wallet_verified=${WALLET}`;
    const router = await start("/events");
    expect(rootData(router).analyticsUserId).toBe(WALLET);

    answerCallback("sid=visitor-sid");
    await postAuthCallback({ signout: true });
    await router.navigate("/help");
    expect(rootLoader).toHaveBeenCalledTimes(2);
    expect(rootData(router).analyticsUserId).toBeNull();
  });

  it("names no account after signing out without navigating, once the page's data is revalidated", async () => {
    cookies = `sid=visitor-sid; dcl_wallet_verified=${WALLET}`;
    const router = await start("/events");
    answerCallback("sid=visitor-sid");
    await postAuthCallback({ signout: true });
    // What `revalidate: true` triggers: the same URL, which root's shouldRevalidate would otherwise skip.
    await router.revalidate();
    expect(rootLoader).toHaveBeenCalledTimes(2);
    expect(rootData(router).analyticsUserId).toBeNull();
  });

  it("reloads root's data once per change: the navigation after the next one does not run it again", async () => {
    const router = await start("/events");
    answerCallback(`sid=visitor-sid; dcl_wallet_verified=${WALLET}`);
    await postAuthCallback({ token: "t" });
    await router.navigate("/help");
    expect(rootLoader).toHaveBeenCalledTimes(2);
    await router.navigate("/events");
    expect(rootLoader).toHaveBeenCalledTimes(2);
    expect(sessionChangePending()).toBe(false);
  });

  it("keeps the change for the navigation when a link prefetch asks shouldRevalidate first", async () => {
    const router = await start("/events");
    answerCallback(`sid=visitor-sid; dcl_wallet_verified=${WALLET}`);
    await postAuthCallback({ token: "t" });
    // react-router's getNewMatchesForLinks (prefetch="intent") asks without navigating.
    const prefetch = {
      currentUrl: new URL("https://example.test/events"),
      currentParams: {},
      nextUrl: new URL("https://example.test/help"),
      nextParams: {},
      defaultShouldRevalidate: true,
    };
    expect(shouldRevalidate(prefetch as never)).toBe(true);
    expect(shouldRevalidate(prefetch as never)).toBe(true);
    await router.navigate("/help");
    expect(rootLoader).toHaveBeenCalledTimes(2);
    expect(rootData(router).analyticsUserId).toBe(WALLET);
  });

  it("does not reload root's data after a post that failed on the way", async () => {
    const router = await start("/events");
    vi.stubGlobal("fetch", vi.fn(async () => { throw new TypeError("Failed to fetch"); }));
    expect(await postAuthCallback({ token: "t" })).toBeNull();
    expect(sessionChangePending()).toBe(false);
    await router.navigate("/help");
    expect(rootLoader).toHaveBeenCalledTimes(1);
  });

  it("keeps the earlier account on a search-only navigation without a sign-in post, which sends no page view either", async () => {
    const router = await start("/events");
    cookies = `sid=visitor-sid; dcl_wallet_verified=${WALLET}`;
    await router.navigate("/events?tab=live");
    expect(rootLoader).toHaveBeenCalledTimes(1);
    expect(rootData(router).analyticsUserId).toBeNull();
  });

  it.each(["/docs/sdk/intro", "/DOCS/sdk/intro"])("reads the session after leaving %s, whose root data has none, for the blog", async (docs) => {
    cookies = `sid=visitor-sid; dcl_wallet_verified=${WALLET}`;
    const router = await start(docs);
    expect(rootData(router)).toMatchObject({ sid: null, analyticsUserId: null });
    await router.navigate("/blog");
    expect(rootLoader).toHaveBeenCalledTimes(2);
    expect(rootData(router)).toMatchObject({ sid: "visitor-sid", analyticsUserId: WALLET });
  });

  it("does not reload root's data between docs pages, or when entering the docs", async () => {
    const router = await start("/docs/sdk/intro");
    await router.navigate("/docs/sdk/next");
    expect(rootLoader).toHaveBeenCalledTimes(1);
    await router.navigate("/blog");
    expect(rootLoader).toHaveBeenCalledTimes(2);
    await router.navigate("/docs/sdk/intro");
    expect(rootLoader).toHaveBeenCalledTimes(2);
    expect(rootData(router).sid).toBe("visitor-sid");
  });
});
