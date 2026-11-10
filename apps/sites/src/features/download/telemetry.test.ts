// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  createAttemptTracker,
  readClick,
  trackArrived,
  trackDownloadClick,
  trackFunnelExit,
  trackPageExit,
  sanitizeReason,
  trackRedirectFailed,
  trackSectionClick,
  type AttemptContext,
} from "./telemetry";

// Field sets of the source Sites events (download contract §1, decentraland/sites@cfdfa1a).
const AUDIT = ["track_called_at", "track_delivered_at", "track_deferred"];
const BASE = [
  "href",
  "os",
  "arch",
  "auth_state",
  "revisit",
  "download_target",
  ...AUDIT,
];

const session = { sid: "sid-1", story: "download" };
const ctx: AttemptContext = {
  href: "https://gateway.example/anonymous/decentraland.exe?anon_user_id=x",
  os: "windows",
  arch: "amd64",
  authState: "anonymous",
  revisit: 0,
  place: "download-page",
  anonUserId: "3f9c1e2a-1b2c-4d3e-8f90-0a1b2c3d4e5f",
  macArch: undefined,
  campaign: { utm_source: "partner" },
};

let calls: { event: string; props: Record<string, unknown>; ctx: unknown }[];
const track = vi.fn(
  (event: string, props: Record<string, unknown>, ctx: unknown) => {
    calls.push({ event, props, ctx });
  },
) as never;

const keys = (props: Record<string, unknown>) => Object.keys(props).sort();

beforeEach(() => {
  calls = [];
  sessionStorage.clear();
});

describe("download lifecycle events", () => {
  it("emits download_started with the source base fields, place, anon id, campaign and started_at", () => {
    createAttemptTracker(ctx, session, track).started(1_000);

    expect(calls[0]!.event).toBe("download_started");
    expect(keys(calls[0]!.props)).toEqual(
      [...BASE, "place", "anon_user_id", "utm_source", "started_at"].sort(),
    );
    expect(calls[0]!.props).toMatchObject({
      os: "Windows",
      track_deferred: true,
      started_at: 1_000,
    });
    expect(calls[0]!.ctx).toBe(session);
  });

  it("emits download_success with delivery mode, gateway request id, bytes and timing", () => {
    const attempt = createAttemptTracker(ctx, session, track);
    attempt.started(1_000);
    attempt.success(
      {
        filename: "decentraland.exe",
        deliveryMode: "streamed",
        gatewayRequestId: "req-9",
        bytes: 42,
      },
      1_500,
    );

    const success = calls[1]!;
    expect(success.event).toBe("download_success");
    expect(success.props).toMatchObject({
      delivery_mode: "streamed",
      gateway_request_id: "req-9",
      bytes_transferred: 42,
      filename: "decentraland.exe",
      started_at: 1_000,
      succeeded_at: 1_500,
      duration_ms: 500,
    });
  });

  it("omits gateway_request_id and bytes when the delivery has none (macOS anchor)", () => {
    createAttemptTracker(
      { ...ctx, os: "macos", arch: "arm64" },
      session,
      track,
    ).success(
      { filename: "decentraland.dmg", deliveryMode: "anchor_native" },
      2_000,
    );
    expect(calls[0]!.props).not.toHaveProperty("gateway_request_id");
    expect(calls[0]!.props).not.toHaveProperty("bytes_transferred");
    expect(calls[0]!.props).toMatchObject({
      os: "macOS",
      started_at: 2_000,
      duration_ms: 0,
    });
  });

  it("emits download_failed with the reason and timing", () => {
    const attempt = createAttemptTracker(ctx, session, track);
    attempt.started(1_000);
    attempt.failed("No download link available", 1_200);
    expect(calls[1]).toMatchObject({
      event: "download_failed",
      props: {
        reason: "No download link available",
        started_at: 1_000,
        failed_at: 1_200,
        duration_ms: 200,
      },
    });
  });

  // Deliberate deviation from source payload parity: source also spreads collectClientFingerprint()'s
  // fp_* fields into these three events (download contract §1). Their only use is re-identification
  // by joining web events to launcher installs through IP and timestamp, and no such join exists in
  // dcl-one's store. Adding them is deferred with the D5 destination decision and a privacy review.
  it("never sends the source client fingerprint fields", () => {
    const attempt = createAttemptTracker(ctx, session, track);
    attempt.started(1);
    attempt.success(
      { filename: "decentraland.exe", deliveryMode: "streamed" },
      2,
    );
    attempt.failed("x", 3);
    for (const call of calls)
      expect(
        Object.keys(call.props).filter((key) => key.startsWith("fp_")),
      ).toEqual([]);
  });

  it("omits place when it is unknown and carries mac_arch when known", () => {
    createAttemptTracker(
      { ...ctx, place: "unknown", macArch: "intel" },
      session,
      track,
    ).started(1);
    expect(calls[0]!.props).not.toHaveProperty("place");
    expect(calls[0]!.props.mac_arch).toBe("intel");
  });
});

describe("click correlation", () => {
  it("joins a desktop installer click to the download events of the next page", () => {
    trackDownloadClick(
      {
        place: "download-page",
        os: "Windows",
        target: "desktop_installer",
        campaign: {},
      },
      session,
      track,
      1_000,
    );
    const click = calls[0]!.props;
    expect(click).toMatchObject({
      event: "Download",
      download_target: "desktop_installer",
    });
    expect(typeof click.click_id).toBe("string");

    createAttemptTracker(ctx, session, track).started(4_000);
    expect(calls[1]!.props).toMatchObject({
      click_id: click.click_id,
      ms_since_click: 3_000,
    });
  });

  it("does not start a join for store exits", () => {
    trackDownloadClick(
      { place: "download-page", os: "iOS", target: "app_store", campaign: {} },
      session,
      track,
      1_000,
    );
    expect(calls[0]!.props).not.toHaveProperty("click_id");
    expect(readClick(1_000)).toBeNull();
  });

  it("forgets a click older than 30 minutes", () => {
    trackDownloadClick(
      { place: "landing-hero", target: "desktop_installer", campaign: {} },
      session,
      track,
      0,
    );
    expect(readClick(30 * 60 * 1000 + 1)).toBeNull();
  });
});

describe("page events", () => {
  it("emits download_success_arrived with place always present", () => {
    trackArrived({ ...ctx, place: "unknown" }, session, track, 5);
    expect(calls[0]!.event).toBe("download_success_arrived");
    expect(keys(calls[0]!.props)).toEqual(
      [
        "os",
        "arch",
        "place",
        "revisit",
        "auth_state",
        "download_target",
        "utm_source",
        ...AUDIT,
      ].sort(),
    );
    expect(calls[0]!.props.place).toBe("unknown");
  });

  it("emits download_funnel_exit and download_page_exit as given", () => {
    trackFunnelExit(
      {
        os: "macOS",
        arch: "arm64",
        place: "landing-hero",
        download_started_fired: true,
        download_success_fired: false,
        download_failed_fired: false,
        ms_on_page: 900,
        revisit: 1,
        auth_state: "authenticated",
      },
      session,
      track,
    );
    trackPageExit(
      { cta_clicked: true, ms_on_page: 1_200, utm_source: "x" },
      session,
      track,
    );
    expect(calls.map((call) => call.event)).toEqual([
      "download_funnel_exit",
      "download_page_exit",
    ]);
  });

  it("caps the redirect failure reason at 200 characters", () => {
    trackRedirectFailed(
      { os: "Windows", arch: "amd64", reason: "x".repeat(500) },
      session,
      track,
    );
    expect(calls[0]!.props).toMatchObject({
      place: "download-page",
      download_target: "desktop_installer",
    });
    expect(String(calls[0]!.props.reason)).toHaveLength(200);
  });
});

describe("section clicks", () => {
  it("send the place with the section or card, the campaign and the audit fields, without event", () => {
    trackSectionClick(
      {
        place: "Landing Explore",
        section: "view_all",
        campaign: { utm_source: "x" },
      },
      session,
      track,
      7,
    );
    trackSectionClick(
      { place: "Landing Weekly Rituals", card: "career-mondays", campaign: {} },
      session,
      track,
      8,
    );

    expect(calls.map((call) => [call.event, keys(call.props)])).toEqual([
      ["Click", ["place", "section", "utm_source", ...AUDIT].sort()],
      ["Click", ["card", "place", ...AUDIT].sort()],
    ]);
    expect(calls[0]!.props).toMatchObject({
      place: "Landing Explore",
      section: "view_all",
    });
  });
});

describe("failure reasons", () => {
  it("drop URLs, which can carry the signed-in identity id, and cap the text", () => {
    const reason = sanitizeReason(
      "Failed to fetch https://gateway.example/3f9c1e2a-identity/decentraland.exe?anon_user_id=x (status 502)",
    );
    expect(reason).toBe("Failed to fetch <url> (status 502)");
    expect(sanitizeReason("y".repeat(500))).toHaveLength(200);
  });

  it("are sanitized on download_failed", () => {
    createAttemptTracker(ctx, session, track).failed(
      "GET https://gateway.example/id-1/x.exe failed",
      10,
    );
    expect(calls[0]!.props.reason).toBe("GET <url> failed");
  });
});
