import { describe, expect, it } from "vitest";

import { eventRecord, placeRecord } from "./fixtures";
import {
  bucketByDay,
  closedHref,
  creatorWallets,
  eventCreator,
  eventHref,
  eventShareUrl,
  legacyEventsHref,
  liveNowItems,
  placeCreator,
  placeHref,
  readDayOffset,
  relativeStart,
  scheduleLine,
  visibleDays,
} from "./view";

const NOW = Date.parse("2099-06-01T12:00:00Z");

describe("liveNowItems", () => {
  it("should show a live event at a busy place with the place's players, then the other busy places", () => {
    const event = eventRecord({ id: "live", live: true, x: 1, y: 95 });
    const items = liveNowItems({
      events: [event],
      places: [
        placeRecord({ id: "mall", positions: ["1,95"], user_count: 18 }),
        placeRecord({ id: "plaza", positions: ["0,0"], user_count: 6 }),
      ],
    });
    expect(items.map((item) => [item.kind, item.users])).toEqual([
      ["event", 18],
      ["place", 6],
    ]);
  });

  it("should leave out a live event whose scene is below the player threshold, and quiet places", () => {
    const items = liveNowItems({
      events: [eventRecord({ live: true, x: 5, y: 5 })],
      places: [placeRecord({ positions: ["5,5"], user_count: 4 })],
    });
    expect(items).toEqual([]);
  });

  it("should honour a lower threshold from the page URL", () => {
    expect(liveNowItems({ events: [], places: [placeRecord({ user_count: 1 })] }, 1)).toHaveLength(1);
  });

  it("should match a World event to its World, not to a Genesis City parcel", () => {
    const event = eventRecord({ live: true, world: true, server: "brai.dcl.eth", x: 0, y: 0 });
    const items = liveNowItems({
      events: [event],
      places: [
        placeRecord({ id: "genesis", positions: ["0,0"], user_count: 9 }),
        placeRecord({ id: "world", world: true, world_name: "brai.dcl.eth", positions: ["0,0"], user_count: 7 }),
      ],
    });
    expect(items.map((item) => (item.kind === "event" ? "event" : item.place.id))).toEqual(["event", "genesis"]);
  });
});

describe("bucketByDay", () => {
  const days = visibleDays(NOW, "UTC", 0, 3).map((day) => day.key);

  it("should place each occurrence of a recurring event in its own day, in start order", () => {
    const weekly = eventRecord({
      id: "daily",
      recurrent: true,
      recurrent_dates: ["2099-06-01T18:00:00Z", "2099-06-02T18:00:00Z", "2099-06-09T18:00:00Z"],
    });
    const early = eventRecord({
      id: "early",
      next_start_at: "2099-06-01T13:00:00Z",
      next_finish_at: "2099-06-01T14:00:00Z",
    });
    const buckets = bucketByDay([weekly, early], days, NOW, "UTC");
    expect(buckets.map((day) => day.map((occurrence) => occurrence.event.id))).toEqual([
      ["early", "daily"],
      ["daily"],
      [],
    ]);
  });

  it("should drop an occurrence that already ended and keep one in progress as live", () => {
    const ended = eventRecord({
      id: "ended",
      next_start_at: "2099-06-01T08:00:00Z",
      next_finish_at: "2099-06-01T09:00:00Z",
    });
    const running = eventRecord({
      id: "running",
      live: true,
      next_start_at: "2099-06-01T11:00:00Z",
      next_finish_at: "2099-06-01T13:00:00Z",
    });
    const [today] = bucketByDay([ended, running], days, NOW, "UTC");
    expect(today?.map((occurrence) => [occurrence.event.id, occurrence.live])).toEqual([["running", true]]);
  });

  it("should group by the viewer's calendar day, not by UTC", () => {
    const late = eventRecord({ next_start_at: "2099-06-02T02:00:00Z", next_finish_at: "2099-06-02T03:00:00Z" });
    const ny = visibleDays(NOW, "America/New_York", 0, 2).map((day) => day.key);
    expect(bucketByDay([late], ny, NOW, "America/New_York").map((day) => day.length)).toEqual([1, 0]);
  });

  it("should leave an event without a start out of every day", () => {
    const undated = eventRecord({ start_at: null, next_start_at: null });
    expect(bucketByDay([undated], days, NOW, "UTC").flat()).toEqual([]);
  });
});

describe("relativeStart", () => {
  it("should count minutes under an hour, then name today, tomorrow or the date", () => {
    expect(relativeStart("2099-06-01T12:08:00Z", NOW, "UTC")).toBe("Starts in 8 mins");
    expect(relativeStart("2099-06-01T17:00:00Z", NOW, "UTC")).toBe("Today 5:00 PM");
    expect(relativeStart("2099-06-02T01:00:00Z", NOW, "UTC")).toBe("Tomorrow 1:00 AM");
    expect(relativeStart("2099-06-05T01:00:00Z", NOW, "UTC")).toBe("Fri, Jun 5 1:00 AM");
  });

  it("should show the time alone for a start already past", () => {
    expect(relativeStart("2099-06-01T11:00:00Z", NOW, "UTC")).toBe("11:00 AM");
  });
});

describe("page URLs", () => {
  it("should open an event or a place while keeping the other parameters, and close back to them", () => {
    expect(eventHref("e/1", "day=4&position=1,2")).toBe("/events?day=4&id=e%2F1");
    expect(placeHref({ coordinates: [1, 2] }, "id=e1&day=4")).toBe("/events?day=4&position=1%2C2");
    expect(placeHref({ coordinates: [0, 0], realm: "brai.dcl.eth" })).toBe("/events?world=brai.dcl.eth");
    expect(closedHref("id=e1&day=4")).toBe("/events?day=4");
    expect(closedHref("world=brai.dcl.eth")).toBe("/events");
  });

  it("should share a live event through its jump page and a later one through this page", () => {
    expect(eventShareUrl("https://example.org", { id: "e1", live: true })).toBe(
      "https://example.org/jump/events?id=e1",
    );
    expect(eventShareUrl("https://example.org", { id: "e1", live: false })).toBe("https://example.org/events?id=e1");
  });

  it("should keep a legacy link's query and turn a detail path into the event to open", () => {
    expect(legacyEventsHref(new URL("https://example.org/whats-on"))).toBe("/events");
    expect(legacyEventsHref(new URL("https://example.org/whats-on/x?env=zone"), "x")).toBe("/events?env=zone&id=x");
  });

  it("should read the day offset only as a whole number of days from 1 to 365", () => {
    for (const [value, expected] of [
      ["4", 4],
      ["-1", 0],
      ["1.5", 0],
      ["abc", 0],
      ["400", 0],
      ["", 0],
    ] as const) {
      expect(readDayOffset(new URLSearchParams({ day: value }))).toBe(expected);
    }
    expect(readDayOffset(new URLSearchParams())).toBe(0);
  });
});

describe("eventCreator", () => {
  it("should link a wallet creator to their profile and name an unknown one", () => {
    expect(eventCreator({ user: "0xAbCdEf0000000000000000000000000000000001", user_name: "Ana" })).toEqual({
      name: "Ana",
      href: "/profile/0xabcdef0000000000000000000000000000000001",
    });
    expect(eventCreator({ user: "not-a-wallet", user_name: null })).toEqual({ name: "Unknown", href: undefined });
  });
});

describe("scheduleLine", () => {
  it("should prefix a recurring event's range with Starting and name its cadence", () => {
    const weekly = eventRecord({ recurrent: true, recurrent_frequency: "WEEKLY", recurrent_interval: 2 });
    expect(scheduleLine(weekly, "UTC")).toMatchObject({
      label: "Starting Mon, Jun 1 · 6:00 PM – 8:00 PM",
      recurrence: "Every 2 weeks",
    });
  });

  it("should have no schedule for an event without a valid start", () => {
    expect(scheduleLine(eventRecord({ start_at: null, next_start_at: "not a date" }), "UTC")).toBeUndefined();
  });
});

describe("creator faces", () => {
  const wallet = "0xabcdef0000000000000000000000000000000001";
  const profiles = { [wallet]: { name: "Ana", avatarUrl: "/profile-images/entities/x/face.png" } };

  it("should show the face a profile read found, and the initial without one", () => {
    expect(eventCreator({ user: wallet.toUpperCase().replace("0X", "0x"), user_name: "Ana" }, profiles).avatarUrl).toBe(
      "/profile-images/entities/x/face.png",
    );
    expect(eventCreator({ user: wallet, user_name: "Ana" }, {}).avatarUrl).toBeUndefined();
  });

  it("should keep the Foundation logo for its places instead of a wallet face", () => {
    const creator = placeCreator({ owner: wallet, contact_name: "Decentraland Foundation" }, profiles);
    expect(creator).toMatchObject({ foundation: true });
    expect(creator.avatarUrl).toBeUndefined();
  });

  it("should collect only wallets from events and places", () => {
    expect(creatorWallets([{ user: wallet }, { user: "someone" }, { owner: null }, { owner: wallet }])).toEqual([
      wallet,
      wallet,
    ]);
  });
});
