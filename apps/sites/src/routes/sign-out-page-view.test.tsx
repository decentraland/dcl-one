// @vitest-environment jsdom
import { act, cleanup, render, waitFor } from "@testing-library/react";
import { createMemoryRouter, Outlet, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

const magic = vi.hoisted(() => ({ end: vi.fn(async () => {}) }));
vi.mock("@/data/lib/auth/magic-session", () => ({ endMagicSession: magic.end }));
const page = vi.hoisted(() => vi.fn());
vi.mock("@/core/lib/telemetry/track", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/core/lib/telemetry/track")>()),
  page,
}));

import { useRoutePageView } from "@/core/lib/telemetry/page-view";
import { restoreTrackedUser } from "@/core/lib/telemetry/tracked-user";
import { readVerifiedWallet } from "@/core/lib/telemetry/session";
import { shouldRevalidate } from "./root";

import { useAuth } from "@/data/lib/auth/context";
import { setIdentity } from "@/data/lib/auth/session";
import { resetSessionChangeForTests, useSessionChangeRevalidation } from "@/data/lib/auth/session-change";

// The production order: `disconnect()` is not awaited before the visitor navigates, so a page view can
// go out while `POST /auth/callback` has not answered yet.

const WALLET = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";

// The cookies the server reads on the next request; the sign-out's answer clears the verified one.
let cookies = "";
let auth: ReturnType<typeof useAuth> | null = null;

function Root() {
  useRoutePageView();
  useSessionChangeRevalidation();
  auth = useAuth();
  return (
    <>
      <p>mounted</p>
      <Outlet />
    </>
  );
}

function setup() {
  const rootLoader = vi.fn(() => ({
    sid: "visitor-sid",
    // What root.tsx's loader reads (routes/root.pageview.test.tsx runs the real one, in Node).
    analyticsUserId: readVerifiedWallet(new Request("https://example.test/", { headers: { cookie: cookies } })),
  }));
  const router = createMemoryRouter(
    [{ id: "root", path: "/", loader: rootLoader, shouldRevalidate, Component: Root, HydrateFallback: () => null, children: [{ path: "*", Component: () => null }] }],
    { initialEntries: ["/events"] },
  );
  const view = render(<RouterProvider router={router} />);
  return { router, rootLoader, view };
}

const viewsOf = (path: string) => page.mock.calls.filter(([name]) => name === path);

describe("page views around a sign-out", () => {
  let answer: ((response: Response) => void) | null;
  let fail: ((error: Error) => void) | null;

  beforeEach(() => {
    page.mockReset();
    restoreTrackedUser();
    resetSessionChangeForTests();
    cookies = `sid=visitor-sid; dcl_wallet_verified=${WALLET}`;
    answer = null;
    fail = null;
    // The callback's answer is held until the test gives it; the answer clears the cookie, as the server's does.
    vi.stubGlobal(
      "fetch",
      vi.fn(
        () =>
          new Promise<Response>((resolve, reject) => {
            answer = (response) => {
              cookies = "sid=visitor-sid";
              resolve(response);
            };
            fail = reject;
          }),
      ),
    );
    setIdentity({
      signer: WALLET,
      ephemeral: { address: `0x${"c".repeat(40)}`, privateKey: `0x${"12".repeat(32)}`, publicKey: `0x04${"ab".repeat(64)}` },
      expiration: new Date(Date.now() + 86_400_000).toISOString(),
      authChain: [{ type: "SIGNER", payload: WALLET, signature: "" }],
    } as never);
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    localStorage.clear();
  });

  it("names no account on a navigation made before the sign-out answers, once, and none after", async () => {
    const { router, rootLoader, view } = setup();
    await view.findByText("mounted");
    await waitFor(() => expect(viewsOf("/events")).toHaveLength(1));
    expect(viewsOf("/events")[0]?.[2]).toEqual({ sid: "visitor-sid", userId: WALLET });

    let signedOut = false;
    act(() => {
      void auth!.disconnect().then(() => {
        signedOut = true;
      });
    });
    await waitFor(() => expect(answer).not.toBeNull());

    // The callback has not answered: the server would still read the verified cookie.
    await act(() => router.navigate("/help"));
    await waitFor(() => expect(viewsOf("/help")).toHaveLength(1));
    expect(viewsOf("/help")[0]?.[2]).toEqual({ sid: "visitor-sid" });
    await waitFor(() => expect(signedOut).toBe(true));

    const loadsBefore = rootLoader.mock.calls.length;
    await act(async () => answer!(Response.json({ ok: true })));
    // The answer reloads root's data in place, now without the wallet...
    await waitFor(() => expect(rootLoader.mock.calls.length).toBe(loadsBefore + 1));
    await act(async () => {});
    // ...and sends no second view of the page it is on.
    expect(viewsOf("/help")).toHaveLength(1);

    await act(() => router.navigate("/events"));
    await waitFor(() => expect(viewsOf("/events")).toHaveLength(2));
    expect(viewsOf("/events")[1]?.[2]).toEqual({ sid: "visitor-sid" });
  });

  it("keeps the account out after a sign-out answered by a delayed 502 that cleared no cookie", async () => {
    const { router, rootLoader, view } = setup();
    await view.findByText("mounted");
    await waitFor(() => expect(viewsOf("/events")).toHaveLength(1));

    act(() => {
      void auth!.disconnect();
    });
    await waitFor(() => expect(answer).not.toBeNull());
    await act(() => router.navigate("/help"));
    await waitFor(() => expect(viewsOf("/help")).toHaveLength(1));

    // A gateway error: no Set-Cookie, so the verified cookie survives.
    const loadsBefore = rootLoader.mock.calls.length;
    const keep = cookies;
    await act(async () => {
      answer!(new Response("bad gateway", { status: 502 }));
      // Restored in the same tick, before any continuation of the answer runs.
      cookies = keep;
    });
    await waitFor(() => expect(rootLoader.mock.calls.length).toBeGreaterThan(loadsBefore));
    await act(async () => {});

    await act(() => router.navigate("/events"));
    await waitFor(() => expect(viewsOf("/events")).toHaveLength(2));
    expect(viewsOf("/events")[1]?.[2]).toEqual({ sid: "visitor-sid" });
    expect(viewsOf("/help")).toHaveLength(1);
  });

  it("names no account after a sign-out whose post failed, and nothing waits on it", async () => {
    const { router, view } = setup();
    await view.findByText("mounted");
    await waitFor(() => expect(page).toHaveBeenCalledTimes(1));

    let signedOut = false;
    act(() => {
      void auth!.disconnect().then(() => {
        signedOut = true;
      });
    });
    await waitFor(() => expect(fail).not.toBeNull());
    await act(async () => fail!(new TypeError("Failed to fetch")));
    await waitFor(() => expect(signedOut).toBe(true));

    // The cookie was never cleared, so root's data still names the wallet: the drop holds.
    await act(() => router.navigate("/help"));
    await waitFor(() => expect(viewsOf("/help")).toHaveLength(1));
    expect(viewsOf("/help")[0]?.[2]).toEqual({ sid: "visitor-sid" });
    await act(() => router.navigate("/events"));
    await waitFor(() => expect(viewsOf("/events")).toHaveLength(2));
    expect(viewsOf("/events")[1]?.[2]).toEqual({ sid: "visitor-sid" });
  });
});
