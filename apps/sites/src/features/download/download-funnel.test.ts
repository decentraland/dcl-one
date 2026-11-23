// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { createLegacyTrackScope } from "@/core/lib/telemetry/legacy-click";
import { track } from "@/core/lib/telemetry/track";
import { dropTrackedUser, restoreTrackedUser } from "@/core/lib/telemetry/tracked-user";

import { DOWNLOAD_CLICK_STORAGE_KEY, downloadCtaClicked, resetDownloadCtaClicked, trackDownloadClick } from "./download-click";
import {
  createDownloadTracker,
  funnelExtra,
  nextRevisit,
  resolveAnonUserId,
  resolveDownloadPlace,
  resolveMacArch,
  sendDownloadFunnelExit,
  sendDownloadSuccessArrived,
  type DownloadAttempt,
} from "./download-funnel";

vi.mock("@/core/lib/telemetry/track", () => ({
  track: vi.fn(),
  legacyAnalyticsReady: () => true,
  whenLegacyAnalyticsReady: (fire: () => void) => fire(),
}));

const NOW = 1_800_000_000_000;
const SESSION = { sid: "s1", userId: "0xabc" };
const CONTEXT = { sid: "s1", userId: "0xabc", legacy: true, conversion: true };
const ANON = "0b7f4c2e-1d3a-4e5f-9a8b-7c6d5e4f3a2b";
const AUDIT = { track_called_at: NOW, track_delivered_at: NOW, track_deferred: true };

const sent = (name: string) => vi.mocked(track).mock.calls.filter(([event]) => event === name);
const only = (name: string) => {
  expect(sent(name)).toHaveLength(1);
  return sent(name)[0]!;
};

function attempt(overrides: Partial<DownloadAttempt> = {}): DownloadAttempt {
  return {
    href: "https://installers.test/Decentraland_installer.dmg",
    os: "macOS",
    arch: "arm64",
    place: "landing-hero",
    anonUserId: ANON,
    authenticated: false,
    revisit: 0,
    extra: { utm_source: "partner", download_target: "desktop_installer" },
    ...overrides,
  };
}

beforeEach(() => {
  vi.useFakeTimers({ now: NOW });
  sessionStorage.clear();
  resetDownloadCtaClicked();
});
afterEach(() => {
  vi.useRealTimers();
  vi.mocked(track).mockReset();
  restoreTrackedUser();
  window.history.replaceState({}, "", "/");
});

describe("the attempt tracker (legacy downloadTracking.ts)", () => {
  it("sends download_started with the extra first, then the base, started_at and the audit fields", () => {
    createDownloadTracker(attempt(), SESSION).started();
    const [, props, ctx] = only("download_started");
    expect(Object.keys(props as object)).toEqual([
      "utm_source",
      "download_target",
      "href",
      "os",
      "arch",
      "auth_state",
      "revisit",
      "place",
      "anon_user_id",
      "started_at",
      "track_called_at",
      "track_delivered_at",
      "track_deferred",
    ]);
    expect(props).toEqual({
      utm_source: "partner",
      download_target: "desktop_installer",
      href: "https://installers.test/Decentraland_installer.dmg",
      os: "macOS",
      arch: "arm64",
      auth_state: "anonymous",
      revisit: 0,
      place: "landing-hero",
      anon_user_id: ANON,
      started_at: NOW,
      ...AUDIT,
    });
    expect(ctx).toEqual(CONTEXT);
  });

  it("times download_success from started, with the delivery first and no bytes or request id it does not have", () => {
    const tracker = createDownloadTracker(attempt({ authenticated: true }), SESSION);
    tracker.started();
    vi.advanceTimersByTime(1200);
    tracker.success("Decentraland_installer.dmg", { mode: "anchor_native" });
    const [, props] = only("download_success");
    expect(Object.keys(props as object).slice(0, 2)).toEqual(["delivery_mode", "utm_source"]);
    expect(props).toMatchObject({
      delivery_mode: "anchor_native",
      auth_state: "authenticated",
      filename: "Decentraland_installer.dmg",
      started_at: NOW,
      succeeded_at: NOW + 1200,
      duration_ms: 1200,
      track_called_at: NOW + 1200,
      track_delivered_at: NOW + 1200,
      track_deferred: true,
    });
    expect(props).not.toHaveProperty("bytes_transferred");
    expect(props).not.toHaveProperty("gateway_request_id");
  });

  it("carries bytes and the gateway request id where a delivery reports them", () => {
    const tracker = createDownloadTracker(attempt(), SESSION);
    tracker.success("f.exe", { mode: "streamed", bytes: 0, requestId: "req-1" });
    const [, props] = only("download_success");
    expect(Object.keys(props as object).slice(0, 2)).toEqual(["delivery_mode", "gateway_request_id"]);
    expect(props).toMatchObject({ bytes_transferred: 0, gateway_request_id: "req-1", duration_ms: 0 });
  });

  it("anchors download_failed at the failure when nothing started, with the reason", () => {
    createDownloadTracker(attempt(), SESSION).failed("No download link available");
    const [, props] = only("download_failed");
    expect(props).toMatchObject({ reason: "No download link available", started_at: NOW, failed_at: NOW, duration_ms: 0, ...AUDIT });
  });

  it("leaves out an unknown place and an absent anonymous id", () => {
    createDownloadTracker(attempt({ place: "unknown", anonUserId: undefined }), SESSION).started();
    const [, props] = only("download_started");
    expect(props).not.toHaveProperty("place");
    expect(props).not.toHaveProperty("anon_user_id");
  });

  it("drops the account once a sign-out has started", () => {
    dropTrackedUser();
    createDownloadTracker(attempt(), SESSION).started();
    expect(only("download_started")[2]).toEqual({ sid: "s1", legacy: true, conversion: true });
  });
});

describe("download_success_arrived", () => {
  const arrival = { os: "Windows", arch: "amd64", place: "unknown", revisit: 2, authenticated: false } as const;

  it("keeps an unknown place, reads this page's utm_* and leaves out a missing correlation", () => {
    window.history.replaceState({}, "", "/download_success?utm_campaign=Fall");
    sendDownloadSuccessArrived(arrival, SESSION);
    const [, props, ctx] = only("download_success_arrived");
    expect(props).toEqual({
      os: "Windows",
      arch: "amd64",
      place: "unknown",
      revisit: 2,
      auth_state: "anonymous",
      utm_campaign: "fall",
      download_target: "desktop_installer",
      ...AUDIT,
    });
    expect(ctx).toEqual(CONTEXT);
  });

  it("joins a fresh click by its click_id and the time since it", () => {
    sessionStorage.setItem(DOWNLOAD_CLICK_STORAGE_KEY, JSON.stringify({ click_id: "c1", clicked_at: NOW - 5000 }));
    sendDownloadSuccessArrived(arrival, SESSION);
    expect(only("download_success_arrived")[1]).toMatchObject({ click_id: "c1", ms_since_click: 5000 });
  });

  it.each([
    ["older than 30 minutes", JSON.stringify({ click_id: "c1", clicked_at: NOW - 30 * 60 * 1000 - 1 })],
    ["malformed", "{not json"],
    ["missing its time", JSON.stringify({ click_id: "c1" })],
  ])("ignores a stored click %s", (_, raw) => {
    sessionStorage.setItem(DOWNLOAD_CLICK_STORAGE_KEY, raw);
    sendDownloadSuccessArrived(arrival, SESSION);
    expect(only("download_success_arrived")[1]).not.toHaveProperty("click_id");
  });
});

describe("download_funnel_exit", () => {
  const exit = {
    os: "macOS",
    arch: "arm64",
    place: "download-page",
    startedFired: true,
    successFired: false,
    failedFired: false,
    msOnPage: 900,
    revisit: 0,
    authenticated: true,
  } as const;

  it("sends the flags in the legacy order, with no audit fields and no utm_*", () => {
    window.history.replaceState({}, "", "/download_success?utm_source=x");
    sessionStorage.setItem(DOWNLOAD_CLICK_STORAGE_KEY, JSON.stringify({ click_id: "c1", clicked_at: NOW }));
    sendDownloadFunnelExit({ ...exit, anonUserId: ANON }, SESSION);
    const [, props, ctx] = only("download_funnel_exit");
    expect(Object.keys(props as object)).toEqual([
      "os",
      "arch",
      "place",
      "download_started_fired",
      "download_success_fired",
      "download_failed_fired",
      "ms_on_page",
      "revisit",
      "auth_state",
      "anon_user_id",
      "click_id",
    ]);
    expect(props).toMatchObject({ download_started_fired: true, auth_state: "authenticated", click_id: "c1", anon_user_id: ANON });
    expect(ctx).toEqual(CONTEXT);
  });

  it("leaves out an absent anonymous id and click", () => {
    sendDownloadFunnelExit(exit, SESSION);
    const [, props] = only("download_funnel_exit");
    expect(props).not.toHaveProperty("anon_user_id");
    expect(props).not.toHaveProperty("click_id");
  });
});

describe("the funnel's inputs", () => {
  it("accepts every legacy place and nothing else", () => {
    expect(resolveDownloadPlace("lets-play-landing")).toBe("lets-play-landing");
    expect(resolveDownloadPlace("Landing Hero")).toBe("unknown");
    expect(resolveDownloadPlace("unknown")).toBe("unknown");
    expect(resolveDownloadPlace(null)).toBe("unknown");
  });

  it("allowlists mac_arch", () => {
    expect(resolveMacArch("intel")).toBe("intel");
    expect(resolveMacArch("x86")).toBeUndefined();
    expect(resolveMacArch(null)).toBeUndefined();
  });

  it("takes a UUID from the URL over the visitor's id, and only UUIDs", () => {
    const other = "1c8e5d3f-2e4b-4f60-8b9c-8d7e6f5a4b3c";
    expect(resolveAnonUserId(other, ANON)).toBe(other);
    expect(resolveAnonUserId("not-a-uuid", ANON)).toBe(ANON);
    expect(resolveAnonUserId(null, "custom-id")).toBeUndefined();
  });

  it("counts revisits per system and architecture in this tab, from 0", () => {
    expect([nextRevisit("macOS", "arm64"), nextRevisit("macOS", "arm64"), nextRevisit("Windows", "amd64")]).toEqual([0, 1, 0]);
    expect(sessionStorage.getItem("downloadSuccess:visits:macOS:arm64")).toBe("2");
  });

  it("counts 0 when storage is blocked", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    expect(nextRevisit("macOS", "arm64")).toBe(0);
    vi.restoreAllMocks();
  });

  it("builds the attempt extra: fingerprint, utm_*, the click, mac_arch, the target last", () => {
    window.history.replaceState({}, "", "/download_success?utm_term=Kw");
    sessionStorage.setItem(DOWNLOAD_CLICK_STORAGE_KEY, JSON.stringify({ click_id: "c1", clicked_at: NOW - 10 }));
    const extra = funnelExtra("apple_silicon");
    expect(Object.keys(extra)).toEqual([
      "fp_screen_width",
      "fp_screen_height",
      "fp_device_pixel_ratio",
      "fp_hardware_concurrency",
      "fp_timezone",
      "fp_language",
      "fp_platform",
      "utm_term",
      "click_id",
      "ms_since_click",
      "mac_arch",
      "download_target",
    ]);
    expect(extra).toMatchObject({ utm_term: "kw", click_id: "c1", ms_since_click: 10, mac_arch: "apple_silicon" });
    expect(funnelExtra(undefined)).not.toHaveProperty("mac_arch");
  });
});

describe("the CTA flag download_page_exit reads (legacy markDownloadCtaClicked)", () => {
  const CLICK = { place: "Creators Hero", event: "Download", download_target: "creator_hub" } as const;

  it("is set by a conversion click, with or without a correlation", () => {
    trackDownloadClick(CLICK, { sid: "s1", scope: createLegacyTrackScope() }, { correlate: false });
    expect(downloadCtaClicked()).toBe(true);
    expect(sessionStorage.getItem(DOWNLOAD_CLICK_STORAGE_KEY)).toBeNull();
  });

  it("is not set by a non-conversion click (the legacy useTrackClick controls)", () => {
    trackDownloadClick({ ...CLICK, place: "Download" }, { sid: "s1", scope: createLegacyTrackScope() }, { conversion: false, correlate: false });
    expect(downloadCtaClicked()).toBe(false);
  });
});
