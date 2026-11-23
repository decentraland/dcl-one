// @vitest-environment jsdom
import { renderHook } from "@testing-library/react";
import { createElement, StrictMode, type ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { legacyAnalyticsReady, track, whenLegacyAnalyticsReady } from "./track";
import { dropTrackedUser, restoreTrackedUser } from "./tracked-user";
import { createLegacyTrackScope, sendLegacyDeferred, trackLegacyClick, trackSectionViewed, useLegacyTrackScope } from "./legacy-click";

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

const SESSION = { sid: "sid-1", scope: createLegacyTrackScope() };

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
    trackLegacyClick({ place: "Creators Why", title: "x" }, { sid: "sid-1", userId: "0xabc", scope: SESSION.scope });
    expect(sent().ctx).toEqual({ sid: "sid-1", userId: "0xabc", legacy: true });
    vi.mocked(track).mockReset();
    dropTrackedUser();
    trackLegacyClick({ place: "Creators Why", title: "x" }, { sid: "sid-1", userId: "0xabc", scope: SESSION.scope });
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

describe("sendLegacyDeferred", () => {
  it("stamps a conversion's delivery with its call time, however long building it took", () => {
    const now = vi.spyOn(Date, "now").mockReturnValueOnce(1000).mockReturnValue(1005);
    const send = vi.fn();
    sendLegacyDeferred(send, SESSION, { conversion: true });
    expect(send.mock.calls[0]![0]).toEqual({ track_called_at: 1000, track_delivered_at: 1000, track_deferred: false });
    now.mockRestore();
  });

  it("stamps a held call's delivery with the time it leaves", () => {
    ready = false;
    const now = vi.spyOn(Date, "now").mockReturnValue(1000);
    const send = vi.fn();
    sendLegacyDeferred(send, { sid: "sid-1", scope: createLegacyTrackScope() });
    now.mockReturnValue(1400);
    becomeReady();
    expect(send.mock.calls[0]![0]).toEqual({ track_called_at: 1000, track_delivered_at: 1400, track_deferred: true });
    now.mockRestore();
  });
});

describe("the emitting component's scope", () => {
  it("drops what is still waiting when the component unmounts, so readiness sends nothing stale", () => {
    ready = false;
    const scope = createLegacyTrackScope();
    trackLegacyClick({ place: "Creators Faqs", title: "faqs-cta" }, { sid: "sid-1", userId: "0xabc", scope });
    trackSectionViewed("Creators Faqs", { sid: "sid-1", scope });
    scope.close();
    becomeReady();
    expect(track).not.toHaveBeenCalled();
  });

  it("refuses a deferral once unmounted", () => {
    const scope = createLegacyTrackScope();
    scope.close();
    trackLegacyClick({ place: "Creators Hero" }, { sid: "sid-1", scope });
    expect(track).not.toHaveBeenCalled();
  });

  it("sends what waited exactly once when the component is still mounted at readiness", () => {
    ready = false;
    const scope = createLegacyTrackScope();
    trackLegacyClick({ place: "Creators Hero" }, { sid: "sid-1", scope });
    becomeReady();
    scope.close();
    expect(sent().props).toMatchObject({ place: "Creators Hero", track_deferred: true });
  });

  it("keeps only the unmounted component's events out, not another's", () => {
    ready = false;
    const left = createLegacyTrackScope();
    const stays = createLegacyTrackScope();
    trackLegacyClick({ place: "Creators Why", title: "left" }, { sid: "sid-1", scope: left });
    trackLegacyClick({ place: "Creators Why", title: "stays" }, { sid: "sid-1", scope: stays });
    left.close();
    becomeReady();
    expect(sent().props).toMatchObject({ title: "stays" });
  });

  it("accepts deferrals again once reopened, as the Strict Mode remount does (close, then open)", () => {
    ready = false;
    const scope = createLegacyTrackScope();
    scope.close();
    scope.open();
    trackLegacyClick({ place: "Creators Hero" }, { sid: "sid-1", scope });
    becomeReady();
    expect(sent().props).toMatchObject({ place: "Creators Hero", track_deferred: true });
  });

  it("drops a held click once the Strict Mode component really unmounts", () => {
    ready = false;
    const strict = ({ children }: { children: ReactNode }) => createElement(StrictMode, null, children);
    const { result, unmount } = renderHook(() => useLegacyTrackScope(), { wrapper: strict });
    trackLegacyClick({ place: "Creators Hero" }, { sid: "sid-1", scope: result.current });
    unmount();
    becomeReady();
    expect(track).not.toHaveBeenCalled();
  });

  it("sends a conversion at once whatever the scope", () => {
    ready = false;
    const scope = createLegacyTrackScope();
    scope.close();
    const send = vi.fn();
    sendLegacyDeferred(send, { sid: "sid-1", scope }, { conversion: true });
    expect(send).toHaveBeenCalledTimes(1);
  });
});
