// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";

import { createLegacyTrackScope } from "@/core/lib/telemetry/legacy-click";
import { track } from "@/core/lib/telemetry/track";

import {
  discoverPlacePayload,
  trackDiscoverClickPlaceCard,
  trackDiscoverFilterCategory,
  trackDiscoverJumpIn,
  trackDiscoverLaunchScene,
} from "./discover-events";

const readiness = vi.hoisted(() => ({ ready: true, pending: [] as Array<() => void> }));
vi.mock("@/core/lib/telemetry/track", () => ({
  track: vi.fn(),
  legacyAnalyticsReady: () => readiness.ready,
  whenLegacyAnalyticsReady: (fire: () => void) => {
    if (readiness.ready) {
      fire();
      return () => {};
    }
    readiness.pending.push(fire);
    return () => {
      readiness.pending = readiness.pending.filter((queued) => queued !== fire);
    };
  },
}));

const PLACE = {
  id: "p1",
  title: "Plaza",
  world: false,
  base_position: "1,2",
  positions: ["1,2", "1,3"],
  world_name: null,
  user_count: 4,
};

afterEach(() => {
  vi.mocked(track).mockReset();
  readiness.ready = true;
  readiness.pending = [];
});

describe("discoverPlacePayload", () => {
  it("carries the place id, title, world flag, base position and user count", () => {
    expect(discoverPlacePayload(PLACE)).toEqual({ place_id: "p1", place_title: "Plaza", world: false, position: "1,2", user_count: 4 });
  });

  it("adds the world name for a world and keeps a count of zero", () => {
    expect(discoverPlacePayload({ ...PLACE, world: true, world_name: "plaza.dcl.eth", user_count: 0 })).toEqual({
      place_id: "p1",
      place_title: "Plaza",
      world: true,
      position: "1,2",
      world_name: "plaza.dcl.eth",
      user_count: 0,
    });
  });

  it("leaves the user count out when the place has none, and passes a missing title through as null", () => {
    const payload = discoverPlacePayload({ ...PLACE, title: null, user_count: null });
    expect(payload).not.toHaveProperty("user_count");
    expect(payload.place_title).toBeNull();
  });

  it("falls back to the first parcel only when the base position is missing, and drops an empty one", () => {
    expect(discoverPlacePayload({ ...PLACE, base_position: undefined as unknown as string }).position).toBe("1,2");
    expect(discoverPlacePayload({ ...PLACE, base_position: undefined as unknown as string, positions: [] })).not.toHaveProperty("position");
    expect(discoverPlacePayload({ ...PLACE, base_position: "" })).not.toHaveProperty("position");
  });
});

describe("the Discover events", () => {
  const session = () => ({ sid: "sid-1", userId: "0xabc", scope: createLegacyTrackScope() });

  it("sends Discover Jump In with the surface as place and the deferred audit fields", () => {
    vi.spyOn(Date, "now").mockReturnValue(1_700_000_000_000);
    trackDiscoverJumpIn(PLACE, "live-card", session());
    expect(track).toHaveBeenCalledWith(
      "Discover Jump In",
      { ...discoverPlacePayload(PLACE), place: "live-card", track_called_at: 1_700_000_000_000, track_delivered_at: 1_700_000_000_000, track_deferred: false },
      { sid: "sid-1", userId: "0xabc", legacy: true },
    );
    vi.restoreAllMocks();
  });

  it("sends Discover Filter Category with only the category and the audit fields", () => {
    trackDiscoverFilterCategory("featured", session());
    const [event, props] = vi.mocked(track).mock.calls[0]!;
    expect(event).toBe("Discover Filter Category");
    expect(Object.keys(props as object).sort()).toEqual(["category", "track_called_at", "track_deferred", "track_delivered_at"]);
  });

  it("sends Discover Launch Scene without href when there is no stream", () => {
    trackDiscoverLaunchScene(undefined, session());
    expect(vi.mocked(track).mock.calls[0]![1]).not.toHaveProperty("href");
  });

  it("drops a card click made before readiness when the page unmounts first", () => {
    readiness.ready = false;
    const s = session();
    trackDiscoverClickPlaceCard(PLACE, s);
    s.scope.close();
    readiness.pending.forEach((fire) => fire());
    expect(track).not.toHaveBeenCalled();
  });

  it("sends a card click made before readiness once, deferred, when the page is still mounted", () => {
    readiness.ready = false;
    trackDiscoverClickPlaceCard(PLACE, session());
    readiness.pending.forEach((fire) => fire());
    expect(track).toHaveBeenCalledTimes(1);
    expect(vi.mocked(track).mock.calls[0]![1]).toMatchObject({ track_deferred: true });
  });
});
