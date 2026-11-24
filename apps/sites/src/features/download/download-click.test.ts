// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { legacyAnalyticsReady, track, whenLegacyAnalyticsReady } from "@/core/lib/telemetry/track";
import { dropTrackedUser, restoreTrackedUser } from "@/core/lib/telemetry/tracked-user";

import { DOWNLOAD_CLICK_STORAGE_KEY, trackDownloadClick, type DownloadClick } from "./download-click";
import { legacyMacArch } from "./legacy-mac-arch";
import { createLegacyTrackScope } from "@/core/lib/telemetry/legacy-click";

// The document's readiness (its first page view) is driven here; `download-click.wire.test.ts` covers the real gates.
let ready = true;
let pending: Array<() => void> = [];
function becomeReady() {
  ready = true;
  const queued = pending;
  pending = [];
  for (const fire of queued) fire();
}

vi.mock("@/core/lib/telemetry/track", () => ({
  track: vi.fn(),
  legacyAnalyticsReady: vi.fn(() => ready),
  whenLegacyAnalyticsReady: vi.fn((fire: () => void) => {
    if (ready) {
      fire();
      return () => {};
    }
    pending.push(fire);
    return () => {
      pending = pending.filter((queued) => queued !== fire);
    };
  }),
}));
vi.mock("./legacy-mac-arch", () => ({ legacyMacArch: vi.fn(() => null) }));

const CLICK: DownloadClick = { place: "Landing Hero", event: "Download", download_target: "desktop_installer" };
const SESSION = { sid: "sid-1", scope: createLegacyTrackScope() };

function sent() {
  expect(track).toHaveBeenCalledTimes(1);
  const [event, props, ctx] = vi.mocked(track).mock.calls[0]!;
  return { event, props: props as Record<string, unknown>, ctx };
}

beforeEach(() => {
  ready = true;
  pending = [];
  vi.mocked(legacyAnalyticsReady).mockClear();
  vi.mocked(whenLegacyAnalyticsReady).mockClear();
  window.history.replaceState(null, "", "/");
  sessionStorage.clear();
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  vi.mocked(track).mockReset();
  vi.mocked(legacyMacArch).mockReset();
  restoreTrackedUser();
});

describe("trackDownloadClick", () => {
  it("sends the legacy Click with its attributes, audit fields and the legacy + conversion gates", () => {
    vi.spyOn(Date, "now").mockReturnValue(1_700_000_000_000);
    trackDownloadClick(CLICK, SESSION);
    const { event, props, ctx } = sent();
    expect(event).toBe("Click");
    expect(props).toMatchObject({
      place: "Landing Hero",
      event: "Download",
      download_target: "desktop_installer",
      track_called_at: 1_700_000_000_000,
      track_delivered_at: 1_700_000_000_000,
      track_deferred: false,
    });
    expect(ctx).toEqual({ sid: "sid-1", legacy: true, conversion: true });
  });

  it("mints click_id and clicked_at and stores them for /download_success", () => {
    trackDownloadClick(CLICK, SESSION);
    const { props } = sent();
    expect(props.click_id).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
    expect(typeof props.clicked_at).toBe("number");
    expect(JSON.parse(sessionStorage.getItem(DOWNLOAD_CLICK_STORAGE_KEY)!)).toEqual({
      click_id: props.click_id,
      clicked_at: props.clicked_at,
    });
  });

  it("still sends the correlation when sessionStorage throws", () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("quota", "QuotaExceededError");
    });
    trackDownloadClick(CLICK, SESSION);
    expect(sent().props.click_id).toEqual(expect.any(String));
  });

  it("writes no correlation when the control opts out", () => {
    sessionStorage.setItem(DOWNLOAD_CLICK_STORAGE_KEY, "previous");
    trackDownloadClick({ ...CLICK, place: "Creators Hero", download_target: "creator_hub" }, SESSION, { correlate: false });
    const { props } = sent();
    expect(props).not.toHaveProperty("click_id");
    expect(props).not.toHaveProperty("clicked_at");
    expect(sessionStorage.getItem(DOWNLOAD_CLICK_STORAGE_KEY)).toBe("previous");
  });

  it("is not a conversion for a useTrackClick control", () => {
    trackDownloadClick(CLICK, SESSION, { conversion: false, correlate: false });
    expect(sent().ctx).toEqual({ sid: "sid-1", legacy: true });
  });

  it("merges the URL's normalized utm_* under the control's own attributes", () => {
    window.history.replaceState(null, "", "/?utm_source=She%20Fi&utm_campaign=Launch!!&place=evil");
    trackDownloadClick(CLICK, SESSION);
    const { props } = sent();
    expect(props).toMatchObject({ utm_source: "she_fi", utm_campaign: "launch", place: "Landing Hero" });
  });

  it("sends no utm_* without campaign parameters or with values that normalize to nothing", () => {
    window.history.replaceState(null, "", "/?utm_source=%21%21&utm_medium=");
    trackDownloadClick(CLICK, SESSION);
    expect(Object.keys(sent().props).filter((k) => k.startsWith("utm_"))).toEqual([]);
  });

  it("adds no mac_arch off a real Mac", () => {
    trackDownloadClick(CLICK, SESSION);
    expect(sent().props).not.toHaveProperty("mac_arch");
  });

  it("adds the legacy mac_arch on a real Mac", () => {
    vi.mocked(legacyMacArch).mockReturnValue("apple_silicon");
    trackDownloadClick(CLICK, SESSION);
    expect(sent().props.mac_arch).toBe("apple_silicon");
  });

  it("sends the verified wallet as userId", () => {
    trackDownloadClick(CLICK, { sid: "sid-1", userId: "0xAbC", scope: SESSION.scope });
    expect(sent().ctx).toEqual({ sid: "sid-1", userId: "0xAbC", legacy: true, conversion: true });
  });

  it("sends no userId for an anonymous visitor", () => {
    trackDownloadClick(CLICK, { sid: "sid-1", userId: "", scope: SESSION.scope });
    expect(sent().ctx).not.toHaveProperty("userId");
  });

  it("sends no userId once a sign-out has started", () => {
    dropTrackedUser();
    trackDownloadClick(CLICK, { sid: "sid-1", userId: "0xabc", scope: SESSION.scope });
    expect(sent().ctx).not.toHaveProperty("userId");
  });

  it("never throws into the click when sending fails", () => {
    vi.mocked(track).mockImplementation(() => {
      throw new Error("boom");
    });
    expect(() => trackDownloadClick(CLICK, SESSION)).not.toThrow();
  });
});

describe("trackDownloadClick track_deferred (the legacy isInitialized)", () => {
  it("sends a conversion at once, deferred, with delivered_at equal to called_at, before the document is ready", () => {
    ready = false;
    vi.spyOn(Date, "now").mockReturnValue(1_700_000_000_000);
    trackDownloadClick(CLICK, SESSION);
    expect(sent().props).toMatchObject({
      track_called_at: 1_700_000_000_000,
      track_delivered_at: 1_700_000_000_000,
      track_deferred: true,
    });
    expect(whenLegacyAnalyticsReady).not.toHaveBeenCalled();
  });

  it("sends a click that is not a conversion at once and not deferred once the document is ready", () => {
    trackDownloadClick(CLICK, SESSION, { conversion: false, correlate: false });
    expect(sent().props.track_deferred).toBe(false);
  });

  it("holds a click that is not a conversion until the document is ready, then sends it deferred with its delivery time", () => {
    ready = false;
    const now = vi.spyOn(Date, "now").mockReturnValue(1_700_000_000_000);
    trackDownloadClick(CLICK, SESSION, { conversion: false, correlate: false });
    expect(track).not.toHaveBeenCalled();

    now.mockReturnValue(1_700_000_000_750);
    becomeReady();
    expect(sent().props).toMatchObject({
      track_called_at: 1_700_000_000_000,
      track_delivered_at: 1_700_000_000_750,
      track_deferred: true,
    });
  });

  it("never sends a click that is not a conversion when the document never becomes ready", () => {
    ready = false;
    trackDownloadClick(CLICK, SESSION, { conversion: false, correlate: false });
    expect(track).not.toHaveBeenCalled();
    expect(pending).toHaveLength(1);
  });

  it("reads the sign-out at delivery, not at the click, for a held click", () => {
    ready = false;
    trackDownloadClick(CLICK, { sid: "sid-1", userId: "0xabc", scope: SESSION.scope }, { conversion: false, correlate: false });
    dropTrackedUser();
    becomeReady();
    expect(sent().ctx).toEqual({ sid: "sid-1", legacy: true });
  });

  it("never throws into the click when reading readiness fails", () => {
    vi.mocked(legacyAnalyticsReady).mockImplementationOnce(() => {
      throw new Error("boom");
    });
    expect(() => trackDownloadClick(CLICK, SESSION)).not.toThrow();
    expect(track).not.toHaveBeenCalled();
  });
});
