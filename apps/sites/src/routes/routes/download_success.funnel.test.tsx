// @vitest-environment jsdom
import { StrictMode, type ComponentProps, type ReactNode } from "react";
import { act, cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider, useLoaderData } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { track } from "@/core/lib/telemetry/track";
import DownloadPage from "@/features/download/DownloadPage";
import { installerSelection } from "@/features/download/installers";
import DownloadSuccessRoute from "./download_success";

// The legacy /download_success funnel (inventory 2.2 and 4.1) through the route, with the sender mocked so each
// payload is compared field by field.

vi.hoisted(() => Object.assign(window, { __vite_plugin_react_preamble_installed__: true, $RefreshReg$: () => {}, $RefreshSig$: () => (t: unknown) => t }));
vi.mock("@/core/lib/telemetry/track", () => ({
  track: vi.fn(),
  legacyAnalyticsReady: () => true,
  whenLegacyAnalyticsReady: (fire: () => void) => fire(),
}));
const auth = vi.hoisted(() => ({ identity: null as unknown, post: vi.fn() }));
vi.mock("@/data/lib/auth/session", () => ({ getIdentity: () => auth.identity }));
vi.mock("@/data/lib/auth/deeplink-identity", () => ({ postIdentityHandoff: auth.post }));
vi.mock("@/core/lib/telemetry/page-view", () => ({ useAnalyticsSid: () => SID, useAnalyticsUserId: () => "" }));
vi.mock("@/features/components/chrome/SitesLayout", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));
vi.mock("@dcl/one-ui/components/AppChrome/chrome-auth", () => ({
  useChromeAuth: () => ({ signedIn: false, account: null, name: null, onSignIn: () => {} }),
}));

const SID = "0b7f4c2e-1d3a-4e5f-9a8b-7c6d5e4f3a2b";
const INSTALLER = (os: string) => `https://installers.test/launcher-rust/Decentraland_installer.${os === "windows" ? "exe" : "dmg"}`;
/** The installer as requested: the legacy page appends the visitor's anonymous id to whatever link it picked. */
const REQUESTED = (os: string) => `${INSTALLER(os)}?anon_user_id=${SID}`;
/** The URL is resolved before the attempt starts, as on the legacy page. */
const settle = () => act(() => vi.advanceTimersByTimeAsync(0));

function data(search: string) {
  const params = new URLSearchParams(search);
  const selection = installerSelection(params);
  return {
    ...selection,
    url: INSTALLER(selection.os),
    locale: "en" as const,
    year: 2026,
    place: params.get("place"),
    macArch: params.get("mac_arch"),
    anonUserId: params.get("anon_user_id"),
    gateway: gateway.current,
    authApiUrl: "https://auth-api.test",
  };
}
const gateway: { current: string | null } = { current: null };

function show(search: string) {
  window.history.replaceState({}, "", `/download_success${search}`);
  const loaderData = data(search);
  const router = createMemoryRouter(
    [
      {
        id: "success",
        path: "*",
        loader: ({ request }) => data(new URL(request.url).search),
        Component: function Success() {
          const current = useLoaderData<typeof data>();
          return <DownloadSuccessRoute {...({ loaderData: current } as ComponentProps<typeof DownloadSuccessRoute>)} />;
        },
      },
    ],
    { initialEntries: [`/download_success${search}`], hydrationData: { loaderData: { success: loaderData } } },
  );
  return {
    router,
    ...render(
      <StrictMode>
        <RouterProvider router={router} />
      </StrictMode>,
    ),
  };
}

const setHidden = (hidden: boolean) =>
  act(() => {
    Object.defineProperty(document, "hidden", { configurable: true, get: () => hidden });
    document.dispatchEvent(new Event("visibilitychange"));
  });
const sent = (name: string) => vi.mocked(track).mock.calls.filter(([event]) => event === name).map(([, props]) => props as Record<string, unknown>);
const names = () => vi.mocked(track).mock.calls.map(([event]) => event);

let requests: string[];
let head: ReturnType<typeof vi.fn>;
beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  sessionStorage.clear();
  requests = [];
  vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (this: HTMLAnchorElement) {
    requests.push(this.href);
  });
  head = vi.fn(async () => new Response(null, { status: 200, headers: { "content-length": "1000" } }));
  vi.stubGlobal("fetch", head);
});
afterEach(() => {
  gateway.current = null;
  auth.identity = null;
  auth.post.mockReset();
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.mocked(track).mockReset();
  Object.defineProperty(document, "hidden", { configurable: true, get: () => false });
  window.history.replaceState({}, "", "/");
});

describe("arriving from a download control", () => {
  it("sends arrived, then started, then success after the macOS hold, once each under StrictMode", async () => {
    show("?os=macOS&place=landing-hero&utm_source=Partner");
    await settle();
    expect(names()).toEqual(["download_success_arrived", "download_started"]);
    expect(requests).toEqual([REQUESTED("macos")]);
    expect(head).toHaveBeenCalledWith(REQUESTED("macos"), expect.objectContaining({ method: "HEAD", credentials: "omit", cache: "no-store" }));
    await act(() => vi.advanceTimersByTimeAsync(800));
    expect(names()).toEqual(["download_success_arrived", "download_started", "download_success"]);

    const [arrived] = sent("download_success_arrived");
    expect(arrived).toMatchObject({ os: "macOS", arch: "arm64", place: "landing-hero", revisit: 0, auth_state: "anonymous", utm_source: "partner" });
    const [success] = sent("download_success");
    expect(success).toMatchObject({
      delivery_mode: "anchor_native",
      href: REQUESTED("macos"),
      os: "macOS",
      arch: "arm64",
      place: "landing-hero",
      anon_user_id: SID,
      filename: "Decentraland_installer.dmg",
      download_target: "desktop_installer",
      utm_source: "partner",
    });
  });

  it("holds the Windows installer for the legacy fixed time and reports it as the anchor fallback", async () => {
    show("?os=Windows&place=download-page&arch=amd64");
    await act(() => vi.advanceTimersByTimeAsync(4000));
    const [success] = sent("download_success");
    expect(success).toMatchObject({ delivery_mode: "anchor_fallback", os: "Windows", arch: "amd64", filename: "Decentraland_installer.exe" });
    expect(success!.duration_ms).toBeGreaterThanOrEqual(4000);
    expect(head).not.toHaveBeenCalled();
  });

  it("sends download_failed with the reason when the browser refuses the download", async () => {
    vi.mocked(HTMLAnchorElement.prototype.click).mockImplementation(() => {
      throw new Error("blocked");
    });
    show("?os=macOS&place=landing-hero");
    await act(() => vi.advanceTimersByTimeAsync(0));
    expect(names()).toEqual(["download_success_arrived", "download_started", "download_failed"]);
    expect(sent("download_failed")[0]).toMatchObject({ reason: "blocked", place: "landing-hero" });
  });

  it("sends no outcome when the visitor leaves before the hold ends", async () => {
    const { unmount } = show("?os=macOS&place=landing-hero");
    unmount();
    await act(() => vi.advanceTimersByTimeAsync(5000));
    expect(names()).toEqual(["download_success_arrived", "download_started"]);
  });

  it("cancels a pending HEAD and the hold when the visitor leaves, sending no outcome", async () => {
    let signal: AbortSignal | undefined;
    head.mockImplementation(
      (_url: string, init: RequestInit) =>
        new Promise((_, reject) => {
          signal = init.signal!;
          signal.addEventListener("abort", () => reject(new DOMException("aborted", "AbortError")));
        }),
    );
    const { unmount } = show("?os=macOS&place=landing-hero");
    await settle();
    expect(signal?.aborted).toBe(false);
    unmount();
    await act(() => vi.advanceTimersByTimeAsync(5000));
    expect(signal?.aborted).toBe(true);
    expect(names()).toEqual(["download_success_arrived", "download_started"]);
  });

  it("cancels the old attempt when the artifact changes on the mounted page; the new one reports once", async () => {
    let signal: AbortSignal | undefined;
    head.mockImplementation((_url: string, init: RequestInit) => {
      signal = init.signal!;
      return new Promise(() => {});
    });
    const { router } = show("?os=macOS&place=landing-hero");
    await settle();
    await act(() => router.navigate("/download_success?os=Windows&place=landing-hero"));
    await settle();
    expect(signal?.aborted).toBe(true);
    expect(requests).toEqual([REQUESTED("macos"), REQUESTED("windows")]);
    await act(() => vi.advanceTimersByTimeAsync(4000));
    expect(names()).toEqual(["download_success_arrived", "download_started", "download_started", "download_success"]);
    expect(sent("download_started").map((p) => p.href)).toEqual([REQUESTED("macos"), REQUESTED("windows")]);
    expect(sent("download_success")[0]).toMatchObject({ href: REQUESTED("windows"), delivery_mode: "anchor_fallback" });
  });

  it("snapshots the funnel on every hide, with the outcomes fired so far", async () => {
    show("?os=macOS&place=landing-hero");
    await settle();
    setHidden(true);
    await act(() => vi.advanceTimersByTimeAsync(800));
    setHidden(false);
    setHidden(true);
    const exits = sent("download_funnel_exit");
    expect(exits).toHaveLength(2);
    expect(exits[0]).toMatchObject({ place: "landing-hero", download_started_fired: true, download_success_fired: false, anon_user_id: SID });
    expect(exits[1]).toMatchObject({ download_success_fired: true, download_failed_fired: false });
    expect(exits[1]).not.toHaveProperty("track_called_at");
  });

  it("counts a revisit of the same system in this tab", () => {
    show("?os=macOS&place=landing-hero");
    cleanup();
    show("?os=macOS&place=landing-hero");
    expect(sent("download_success_arrived").map((p) => p.revisit)).toEqual([0, 1]);
  });
});

describe("expected negatives (peer plan)", () => {
  it("a direct arrival (no known place) reports its arrival with place unknown, leaves place off the attempt and never sends an exit", async () => {
    show("?os=macOS");
    await settle();
    setHidden(true);
    expect(sent("download_success_arrived")[0]).toMatchObject({ place: "unknown" });
    expect(sent("download_started")[0]).not.toHaveProperty("place");
    expect(sent("download_funnel_exit")).toHaveLength(0);
  });

  it("the footer retry starts its own attempt and sends no Click", async () => {
    show("?os=macOS&place=landing-hero");
    await act(() => vi.advanceTimersByTimeAsync(800));
    vi.mocked(track).mockClear();
    await userEvent.setup({ advanceTimers: vi.advanceTimersByTime }).click(screen.getByRole("link", { name: "Try again" }));
    expect(names()).toEqual(["download_started"]);
    expect(sent("download_started")[0]).toMatchObject({ place: "download-success-footer" });
    await act(() => vi.advanceTimersByTimeAsync(800));
    expect(names()).toEqual(["download_started", "download_success"]);
    expect(names()).not.toContain("Click");
  });
});

describe("click to exit: one click_id across the funnel (Codex fixture)", () => {
  it("joins the /download Click to arrived, started, success and the exit by the same click_id", async () => {
    window.history.replaceState({}, "", "/download?utm_source=partner");
    const page = render(
      <DownloadPage
        primary={{ os: "macos", arch: "arm64", url: INSTALLER("macos") }}
        secondary={{ os: "windows", arch: "amd64", url: INSTALLER("windows") }}
        android={false}
        downloads={null}
        sid={SID}
        search="utm_source=partner"
      />,
    );
    const cta = screen.getByRole("link", { name: "Download for macOS" });
    cta.addEventListener("click", (e) => e.preventDefault());
    await userEvent.setup({ advanceTimers: vi.advanceTimersByTime }).click(cta);
    const href = cta.getAttribute("href")!;
    page.unmount();

    show(href.slice(href.indexOf("?")));
    await act(() => vi.advanceTimersByTimeAsync(800));
    setHidden(true);

    const [click] = sent("Click");
    expect(click).toMatchObject({ place: "Download", download_target: "desktop_installer", click_id: expect.any(String) });
    const clickId = click!.click_id;
    for (const name of ["download_success_arrived", "download_started", "download_success", "download_funnel_exit"]) {
      expect(sent(name).map((p) => p.click_id), name).toEqual([clickId]);
    }
    expect(sent("download_funnel_exit")[0]).toMatchObject({ place: "download-page", download_success_fired: true });
    expect(sent("download_started")[0]).toMatchObject({ utm_source: "partner", ms_since_click: expect.any(Number) });
  });
});

describe("the download gateway (legacy calculateCDNReleaseLinksWithIdentity)", () => {
  const GATEWAY = "https://gateway.test";
  beforeEach(() => {
    gateway.current = GATEWAY;
  });

  it("sends a signed-out visitor through the anonymous route with the campaign id, without asking for a token", async () => {
    show("?os=Windows&place=download-page");
    await settle();
    expect(auth.post).not.toHaveBeenCalled();
    expect(requests).toEqual([`${GATEWAY}/anonymous/decentraland.exe?anon_user_id=${SID}`]);
    await act(() => vi.advanceTimersByTimeAsync(4000));
    expect(sent("download_success")[0]).toMatchObject({ filename: "decentraland.exe", href: requests[0] });
  });

  it("sends a signed-in visitor through the identity route with the token from POST /identities", async () => {
    auth.identity = { signer: "0xabc" };
    auth.post.mockResolvedValue({ identityId: "token-1", expiration: "" });
    show("?os=macOS&arch=arm64&place=landing-hero");
    await settle();
    expect(auth.post).toHaveBeenCalledWith(auth.identity, "https://auth-api.test", { signal: expect.any(AbortSignal) });
    expect(requests).toEqual([`${GATEWAY}/token-1/decentraland.dmg?anon_user_id=${SID}`]);
  });

  it("keeps an Intel Mac on the plain installer on the identity route, and on the gateway on the anonymous one", async () => {
    auth.identity = { signer: "0xabc" };
    auth.post.mockResolvedValue({ identityId: "token-1", expiration: "" });
    show("?os=macOS&arch=amd64&place=landing-hero");
    await settle();
    expect(requests[0]).toBe(`${INSTALLER("macos")}?anon_user_id=${SID}`);
    cleanup();
    requests.length = 0;
    auth.identity = null;
    show("?os=macOS&arch=amd64&place=landing-hero");
    await settle();
    expect(requests[0]).toBe(`${GATEWAY}/anonymous/decentraland.dmg?anon_user_id=${SID}`);
  });

  it("falls back to the anonymous route when the token request fails", async () => {
    auth.identity = { signer: "0xabc" };
    auth.post.mockRejectedValue(new Error("Auth API responded with status 500"));
    show("?os=Windows&place=download-page");
    await settle();
    expect(requests).toEqual([`${GATEWAY}/anonymous/decentraland.exe?anon_user_id=${SID}`]);
    expect(sent("download_failed")).toHaveLength(0);
  });

  it("gives up on a token request after 5 seconds and takes the anonymous route", async () => {
    auth.identity = { signer: "0xabc" };
    auth.post.mockImplementation(
      (_identity: unknown, _url: string, { signal }: { signal: AbortSignal }) =>
        new Promise((_, reject) => signal.addEventListener("abort", () => reject(signal.reason))),
    );
    show("?os=Windows&place=download-page");
    await settle();
    expect(requests).toEqual([]);
    await act(() => vi.advanceTimersByTimeAsync(5000));
    expect(requests).toEqual([`${GATEWAY}/anonymous/decentraland.exe?anon_user_id=${SID}`]);
  });

  it("starts nothing when the visitor leaves while the token is pending", async () => {
    auth.identity = { signer: "0xabc" };
    let signal: AbortSignal | undefined;
    auth.post.mockImplementation((_i: unknown, _u: string, opts: { signal: AbortSignal }) => {
      signal = opts.signal;
      return new Promise(() => {});
    });
    const { unmount } = show("?os=Windows&place=download-page");
    await settle();
    unmount();
    await settle();
    expect(signal?.aborted).toBe(true);
    expect(requests).toEqual([]);
    expect(names()).toEqual(["download_success_arrived"]);
  });

  it("carries the deep link and referrer the gateway bakes in, never the click_id", async () => {
    sessionStorage.setItem("downloadFunnel:lastClick", JSON.stringify({ click_id: "c-1", clicked_at: Date.now() }));
    show("?os=Windows&place=download-page&position=10.20&realm=foo.dcl.eth&referrer=0xAbCdEf0123456789abcdef0123456789ABCDEF01");
    await settle();
    expect(requests).toEqual([
      `${GATEWAY}/anonymous/decentraland.exe?anon_user_id=${SID}&position=10%2C20&realm=foo.dcl.eth&referrer=0xabcdef0123456789abcdef0123456789abcdef01`,
    ]);
    expect(requests[0]).not.toContain("c-1");
    expect(sent("download_started")[0]).toMatchObject({ click_id: "c-1", anon_user_id: SID });
  });
});
