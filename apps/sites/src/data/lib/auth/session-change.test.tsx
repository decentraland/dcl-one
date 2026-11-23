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

import { restoreTrackedUser, trackedUserDropped } from "@/core/lib/telemetry/tracked-user";

import {
  AUTH_CALLBACK_PATH,
  beginSignOut,
  markSessionChanged,
  postAuthCallback,
  resetSessionChangeForTests,
  sessionChangePending,
  signedOutElsewhere,
  useSessionChangeRevalidation,
} from "./session-change";

function Root() {
  useSessionChangeRevalidation();
  return (
    <>
      <p>mounted</p>
      <Outlet />
    </>
  );
}

describe("session change", () => {
  beforeEach(() => {
    restoreTrackedUser();
    resetSessionChangeForTests();
  });
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  const CHALLENGE = "a".repeat(43);

  /** /auth/callback: a challenge for the sign-in to answer, then `answer` to the post that changes the cookie. */
  function callback(answer: () => Response) {
    const fetchMock = vi.fn(async (_url: string, init: RequestInit) =>
      JSON.parse(String(init.body)).challenge === true ? Response.json({ ok: true, challenge: CHALLENGE }) : answer(),
    );
    vi.stubGlobal("fetch", fetchMock);
    return fetchMock;
  }

  it("marks the change for any answer of /auth/callback, a refusal included (it clears the cookie)", async () => {
    const fetchMock = callback(() => Response.json({ ok: false }, { status: 401 }));
    const response = await postAuthCallback({ token: "expired" });
    expect(response).toEqual({ ok: false, status: 401 });
    expect(fetchMock).toHaveBeenNthCalledWith(1, AUTH_CALLBACK_PATH, expect.objectContaining({ method: "POST", body: '{"challenge":true}' }));
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      AUTH_CALLBACK_PATH,
      expect.objectContaining({ method: "POST", body: `{"token":"expired","challenge":"${CHALLENGE}"}` }),
    );
    expect(sessionChangePending()).toBe(true);
  });

  it.each([
    ["a refusal", () => Response.json({ ok: false }, { status: 403 })],
    ["an answer without a challenge", () => Response.json({ ok: true })],
    ["an answer that is not JSON", () => new Response("<html>", { status: 200 })],
  ])("posts no sign-in and marks no change when the challenge gets %s", async (_name, challengeAnswer) => {
    const fetchMock = vi.fn(async () => challengeAnswer());
    vi.stubGlobal("fetch", fetchMock);
    const response = await postAuthCallback({ token: "t" });
    expect(response?.ok).toBe(false);
    expect(fetchMock).toHaveBeenCalledOnce();
    expect(sessionChangePending()).toBe(false);
  });

  it("holds the tabs' shared lock for each cookie change, so another tab's post never interleaves", async () => {
    callback(() => Response.json({ ok: true }));
    const held: string[] = [];
    const request = vi.fn(async (name: string, run: () => Promise<unknown>) => {
      held.push(name);
      return run();
    });
    vi.stubGlobal("navigator", { ...navigator, locks: { request } });
    expect(await postAuthCallback({ token: "t" })).toEqual({ ok: true, status: 200 });
    await postAuthCallback({ signout: true });
    expect(held).toEqual(["dcl:auth:verified-wallet", "dcl:auth:verified-wallet"]);
  });

  it("asks a sign-out for no challenge", async () => {
    const fetchMock = callback(() => Response.json({ ok: true }));
    await postAuthCallback({ signout: true });
    expect(fetchMock).toHaveBeenCalledExactlyOnceWith(AUTH_CALLBACK_PATH, expect.objectContaining({ body: '{"signout":true}' }));
  });

  it("lets only a successful sign-in restore an account a sign-out left out", async () => {
    beginSignOut();
    callback(() => new Response("error", { status: 500 }));
    await postAuthCallback({ token: "t" });
    expect(trackedUserDropped()).toBe(true);
    callback(() => Response.json({ ok: false }, { status: 401 }));
    await postAuthCallback({ token: "expired" });
    expect(trackedUserDropped()).toBe(true);
    callback(() => Response.json({ ok: true }));
    await postAuthCallback({ token: "good" });
    expect(trackedUserDropped()).toBe(false);
  });

  it.each([
    ["a success", 200],
    ["a refusal", 401],
  ])("releases the answer's body on %s, which no caller reads", async (_label, status) => {
    const answer = Response.json({ ok: status === 200 }, { status });
    const cancel = vi.spyOn(answer.body!, "cancel");
    callback(() => answer);
    expect(await postAuthCallback({ token: "t" })).toEqual({ ok: status === 200, status });
    expect(cancel).toHaveBeenCalledOnce();
  });

  it("does not mark a post that never reached the server", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => { throw new TypeError("Failed to fetch"); }));
    expect(await postAuthCallback({ signout: true }, { keepalive: true, revalidate: true })).toBeNull();
    expect(sessionChangePending()).toBe(false);
  });

  function renderRoot() {
    const loads = { count: 0 };
    const router = createMemoryRouter(
      [
        {
          id: "root",
          path: "/",
          loader: () => ({ read: ++loads.count }),
          shouldRevalidate: ({ currentUrl, nextUrl, defaultShouldRevalidate }) =>
            sessionChangePending() || (currentUrl.pathname !== nextUrl.pathname && defaultShouldRevalidate),
          Component: Root,
          // As the app's root error page, it replaces the component that mounts the hook.
          ErrorBoundary: () => <p>error page</p>,
          HydrateFallback: () => null,
          children: [
            {
              path: "broken",
              loader: () => {
                throw new Error("page failed");
              },
            },
            { path: "*", Component: () => null },
          ],
        },
      ],
      { initialEntries: ["/events"] },
    );
    const view = render(<RouterProvider router={router} />);
    return { loads, view, router };
  }

  it("reloads root's data in place for a change that asks to, then holds no change", async () => {
    const { loads, view, router } = renderRoot();
    await view.findByText("mounted");
    expect(loads.count).toBe(1);

    act(() => markSessionChanged({ revalidate: true }));
    await waitFor(() => expect(loads.count).toBe(2));
    await waitFor(() => expect(sessionChangePending()).toBe(false));
    router.dispose();
  });

  it("keeps a change without `revalidate` for the next navigation", async () => {
    const { loads, view, router } = renderRoot();
    await view.findByText("mounted");

    act(() => markSessionChanged());
    await act(async () => {});
    expect(loads.count).toBe(1);
    expect(sessionChangePending()).toBe(true);

    await act(() => router.navigate("/places"));
    await waitFor(() => expect(loads.count).toBe(2));
    await waitFor(() => expect(sessionChangePending()).toBe(false));
    router.dispose();
  });

  it("runs a reload asked for before the router mounted once it mounts", async () => {
    markSessionChanged({ revalidate: true });
    const { loads, router } = renderRoot();
    await waitFor(() => expect(loads.count).toBe(2));
    await waitFor(() => expect(sessionChangePending()).toBe(false));
    router.dispose();
  });

  it("applies a change once the hook mounts again after the root error page, so later navigations keep root's data", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    const { loads, view, router } = renderRoot();
    await view.findByText("mounted");

    act(() => markSessionChanged());
    // The navigation reloads root's data, but its page fails and the error page replaces the hook.
    await act(() => router.navigate("/broken"));
    await view.findByText("error page");
    expect(loads.count).toBe(2);

    await act(() => router.navigate("/events"));
    await view.findByText("mounted");
    await waitFor(() => expect(sessionChangePending()).toBe(false));
    const afterRecovery = loads.count;
    await act(() => router.navigate("/places"));
    expect(loads.count).toBe(afterRecovery);
    router.dispose();
    vi.mocked(console.error).mockRestore();
  });

  it("keeps the change when root's own reload fails, and applies it on the next one that succeeds", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    let failRoot = false;
    const router = createMemoryRouter(
      [
        {
          id: "root",
          path: "/",
          loader: () => {
            if (failRoot) throw new Error("root failed");
            return { at: Date.now() };
          },
          shouldRevalidate: () => sessionChangePending(),
          Component: Root,
          ErrorBoundary: () => <p>error page</p>,
          HydrateFallback: () => null,
          children: [{ path: "*", Component: () => null }],
        },
      ],
      { initialEntries: ["/events"] },
    );
    const view = render(<RouterProvider router={router} />);
    await view.findByText("mounted");

    act(() => markSessionChanged());
    failRoot = true;
    await act(() => router.navigate("/places"));
    await view.findByText("error page");
    expect(sessionChangePending()).toBe(true);

    failRoot = false;
    await act(() => router.navigate("/help"));
    await view.findByText("mounted");
    await waitFor(() => expect(sessionChangePending()).toBe(false));
    router.dispose();
    vi.mocked(console.error).mockRestore();
  });

  it("leaves the account out and reloads root's data in place after a sign-out in another tab", async () => {
    const { loads, view, router } = renderRoot();
    await view.findByText("mounted");

    act(() => signedOutElsewhere());
    expect(trackedUserDropped()).toBe(true);
    await waitFor(() => expect(loads.count).toBe(2));
    await waitFor(() => expect(sessionChangePending()).toBe(false));
    // No server answer reached this tab, so the account stays left out until a sign-in here.
    expect(trackedUserDropped()).toBe(true);
    router.dispose();
  });

  it("does not reload on mount for a change before it that did not ask to", async () => {
    markSessionChanged();
    const { loads, view, router } = renderRoot();
    await view.findByText("mounted");
    await act(async () => {});
    expect(loads.count).toBe(1);
    expect(sessionChangePending()).toBe(true);
    router.dispose();
  });
});
