// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { legacyAnalyticsReady, track, whenLegacyAnalyticsReady } from "./track";
import { dropTrackedUser, restoreTrackedUser } from "./tracked-user";
import { trackLegacyClick, trackSectionViewed } from "./legacy-click";

// The document's readiness (its first page view) is driven here; `legacy-click.wire.test.ts` covers the real gates.
let ready = true;
let pending: Array<() => void> = [];
function becomeReady() {
  ready = true;
  const queued = pending;
  pending = [];
  for (const fire of queued) fire();
}

vi.mock("./track", () => ({
  track: vi.fn(),
  legacyAnalyticsReady: vi.fn(() => ready),
  whenLegacyAnalyticsReady: vi.fn((fire: () => void) => (ready ? fire() : pending.push(fire))),
}));

const SESSION = { sid: "sid-1" };

function sent() {
  expect(track).toHaveBeenCalledTimes(1);
  const [event, props, ctx] = vi.mocked(track).mock.calls[0]!;
  return { event, props: props as Record<string, unknown>, ctx };
}

function stubMobile(matches: boolean) {
  vi.stubGlobal("matchMedia", (query: string) => ({ matches: matches && query === "(max-width: 767.95px)", media: query }));
}

beforeEach(() => {
  ready = true;
  pending = [];
  vi.mocked(legacyAnalyticsReady).mockClear();
  vi.mocked(whenLegacyAnalyticsReady).mockClear();
  window.history.replaceState(null, "", "/");
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  vi.mocked(track).mockReset();
  restoreTrackedUser();
});

describe("trackLegacyClick", () => {
  it("sends the legacy Click with exactly the control's attributes and the audit fields", () => {
    vi.spyOn(Date, "now").mockReturnValue(1_700_000_000_000);
    trackLegacyClick({ place: "Creators Create", card: "design-unique-wearables", tab: "Regular", title: "Creating Wearables" }, SESSION);
    const { event, props, ctx } = sent();
    expect(event).toBe("Click");
    expect(props).toEqual({
      place: "Creators Create",
      card: "design-unique-wearables",
      tab: "Regular",
      title: "Creating Wearables",
      track_called_at: 1_700_000_000_000,
      track_delivered_at: 1_700_000_000_000,
      track_deferred: false,
    });
    expect(ctx).toEqual({ sid: "sid-1", legacy: true });
  });

  it("sends no event, download_target, mac_arch or click correlation", () => {
    trackLegacyClick({ place: "Landing Catch The Vibe" }, SESSION);
    const { props } = sent();
    for (const key of ["event", "download_target", "mac_arch", "click_id", "clicked_at", "os"]) expect(props).not.toHaveProperty(key);
  });

  it("merges the URL's normalized utm_* under the control's attributes", () => {
    window.history.replaceState(null, "", "/create?utm_source=Partner%20Site&utm_campaign=&utm_term=Avatars");
    trackLegacyClick({ place: "Creators Hero", title: "scroll-to-why" }, SESSION);
    const { props } = sent();
    expect(props).toMatchObject({ utm_source: "partner_site", utm_term: "avatars", place: "Creators Hero", title: "scroll-to-why" });
    expect(props).not.toHaveProperty("utm_campaign");
  });

  it("leaves out an attribute whose value is empty or missing, as the legacy data-* reader did", () => {
    trackLegacyClick({ place: "Landing Explore", card: "", section: undefined }, SESSION);
    const { props } = sent();
    expect(props).not.toHaveProperty("card");
    expect(props).not.toHaveProperty("section");
  });

  it("names the verified wallet as userId, and drops it once a sign-out has started", () => {
    trackLegacyClick({ place: "Creators Why", title: "x" }, { sid: "sid-1", userId: "0xabc" });
    expect(sent().ctx).toEqual({ sid: "sid-1", userId: "0xabc", legacy: true });
    vi.mocked(track).mockReset();
    dropTrackedUser();
    trackLegacyClick({ place: "Creators Why", title: "x" }, { sid: "sid-1", userId: "0xabc" });
    expect(sent().ctx).toEqual({ sid: "sid-1", legacy: true });
  });

  it("holds a click made before the document is ready and sends it deferred once it is", () => {
    ready = false;
    const now = vi.spyOn(Date, "now").mockReturnValue(1_700_000_000_000);
    trackLegacyClick({ place: "Landing Weekly Rituals", card: "watch-party" }, SESSION);
    expect(track).not.toHaveBeenCalled();
    now.mockReturnValue(1_700_000_000_250);
    becomeReady();
    expect(sent().props).toMatchObject({
      card: "watch-party",
      track_called_at: 1_700_000_000_000,
      track_delivered_at: 1_700_000_000_250,
      track_deferred: true,
    });
  });

  it("never throws into the click when telemetry fails", () => {
    vi.mocked(whenLegacyAnalyticsReady).mockImplementationOnce(() => {
      throw new Error("boom");
    });
    expect(() => trackLegacyClick({ place: "Creators Faqs", title: "faqs-cta" }, SESSION)).not.toThrow();
    expect(track).not.toHaveBeenCalled();
  });
});

describe("trackSectionViewed", () => {
  it("sends section_viewed, mobile and the audit fields, and no utm_*", () => {
    window.history.replaceState(null, "", "/create?utm_source=partner");
    stubMobile(false);
    vi.spyOn(Date, "now").mockReturnValue(1_700_000_000_000);
    trackSectionViewed("Creators Learn", SESSION);
    const { event, props, ctx } = sent();
    expect(event).toBe("Section Viewed");
    expect(props).toEqual({
      section_viewed: "Creators Learn",
      mobile: false,
      track_called_at: 1_700_000_000_000,
      track_delivered_at: 1_700_000_000_000,
      track_deferred: false,
    });
    expect(ctx).toEqual({ sid: "sid-1", legacy: true });
  });

  it("is mobile below ui2's 768px breakpoint", () => {
    stubMobile(true);
    trackSectionViewed("Creators Why", SESSION);
    expect(sent().props).toMatchObject({ mobile: true });
  });

  it("is not mobile where the browser has no matchMedia", () => {
    vi.stubGlobal("matchMedia", undefined);
    trackSectionViewed("Creators Why", SESSION);
    expect(sent().props).toMatchObject({ mobile: false });
  });

  it("waits for the document to be ready, as the legacy queue did", () => {
    ready = false;
    stubMobile(false);
    trackSectionViewed("Creators Faqs", SESSION);
    expect(track).not.toHaveBeenCalled();
    becomeReady();
    expect(sent().props).toMatchObject({ section_viewed: "Creators Faqs", track_deferred: true });
  });
});
