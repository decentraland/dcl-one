import { beforeEach, describe, expect, it } from "vitest";

import type { Event } from "@/data/lib/catalyst/places/events";

import { scheduleLine } from "../components/event-schedule";
import { eventRecord, placeRecord } from "./fixtures";
import {
  bucketByDay,
  closedHref,
  creatorWallets,
  dayLabel,
  eventCreator,
  eventHref,
  eventPageShareUrl,
  legacyEventsHref,
  liveNowItems,
  placeCreator,
  placeHref,
  readDayOffset,
  relativeStart,
  visibleDays,
} from "./view";

const NOW = Date.parse("2099-06-01T12:00:00Z");

describe("liveNowItems", () => {
  let data: Parameters<typeof liveNowItems>[0];

  describe("when a live event is at a busy place and another place is busy", () => {
    beforeEach(() => {
      data = {
        events: [eventRecord({ id: "live", live: true, x: 1, y: 95 })],
        places: [
          placeRecord({ id: "mall", positions: ["1,95"], user_count: 18 }),
          placeRecord({ id: "plaza", positions: ["0,0"], user_count: 6 }),
        ],
      };
    });

    it("should show the live event with the place's players, then the other busy places", () => {
      expect(liveNowItems(data).map((item) => [item.kind, item.users])).toEqual([
        ["event", 18],
        ["place", 6],
      ]);
    });
  });

  describe("when the live event's scene and the places are below the player threshold", () => {
    beforeEach(() => {
      data = {
        events: [eventRecord({ live: true, x: 5, y: 5 })],
        places: [placeRecord({ positions: ["5,5"], user_count: 4 })],
      };
    });

    it("should leave out the live event and the quiet places", () => {
      expect(liveNowItems(data)).toEqual([]);
    });
  });

  describe("when the page URL sets a lower threshold", () => {
    beforeEach(() => {
      data = { events: [], places: [placeRecord({ user_count: 1 })] };
    });

    it("should honour it", () => {
      expect(liveNowItems(data, 1)).toHaveLength(1);
    });
  });

  describe("when a live World event shares coordinates with a Genesis City parcel", () => {
    beforeEach(() => {
      data = {
        events: [eventRecord({ live: true, world: true, server: "brai.dcl.eth", x: 0, y: 0 })],
        places: [
          placeRecord({ id: "genesis", positions: ["0,0"], user_count: 9 }),
          placeRecord({ id: "world", world: true, world_name: "brai.dcl.eth", positions: ["0,0"], user_count: 7 }),
        ],
      };
    });

    it("should match the event to its World, not to the parcel", () => {
      expect(liveNowItems(data).map((item) => (item.kind === "event" ? "event" : item.place.id))).toEqual(["event", "genesis"]);
    });
  });
});

describe("bucketByDay", () => {
  let days: string[];
  let events: Event[];

  beforeEach(() => {
    days = visibleDays(NOW, "UTC", 0, 3).map((day) => day.key);
  });

  describe("when a recurring event has occurrences on several days", () => {
    beforeEach(() => {
      events = [
        eventRecord({
          id: "daily",
          recurrent: true,
          recurrent_dates: ["2099-06-01T18:00:00Z", "2099-06-02T18:00:00Z", "2099-06-09T18:00:00Z"],
        }),
        eventRecord({
          id: "early",
          next_start_at: "2099-06-01T13:00:00Z",
          next_finish_at: "2099-06-01T14:00:00Z",
        }),
      ];
    });

    it("should place each occurrence in its own day, in start order", () => {
      expect(bucketByDay(events, days, NOW, "UTC").map((day) => day.map((occurrence) => occurrence.event.id))).toEqual([
        ["early", "daily"],
        ["daily"],
        [],
      ]);
    });
  });

  describe("when one occurrence already ended and another is in progress", () => {
    beforeEach(() => {
      events = [
        eventRecord({
          id: "ended",
          next_start_at: "2099-06-01T08:00:00Z",
          next_finish_at: "2099-06-01T09:00:00Z",
        }),
        eventRecord({
          id: "running",
          live: true,
          next_start_at: "2099-06-01T11:00:00Z",
          next_finish_at: "2099-06-01T13:00:00Z",
        }),
      ];
    });

    it("should drop the ended one and keep the one in progress as live", () => {
      const [today] = bucketByDay(events, days, NOW, "UTC");
      expect(today?.map((occurrence) => [occurrence.event.id, occurrence.live])).toEqual([["running", true]]);
    });
  });

  describe("when the viewer's calendar day differs from the UTC day", () => {
    beforeEach(() => {
      events = [eventRecord({ next_start_at: "2099-06-02T02:00:00Z", next_finish_at: "2099-06-02T03:00:00Z" })];
      days = visibleDays(NOW, "America/New_York", 0, 2).map((day) => day.key);
    });

    it("should group by the viewer's calendar day", () => {
      expect(bucketByDay(events, days, NOW, "America/New_York").map((day) => day.length)).toEqual([1, 0]);
    });
  });

  describe("when an event has no start", () => {
    beforeEach(() => {
      events = [eventRecord({ start_at: null, next_start_at: null })];
    });

    it("should leave it out of every day", () => {
      expect(bucketByDay(events, days, NOW, "UTC").flat()).toEqual([]);
    });
  });
});

describe("relativeStart", () => {
  describe("when the start is under an hour away", () => {
    it("should count the minutes", () => {
      expect(relativeStart("2099-06-01T12:08:00Z", NOW, "UTC")).toBe("Starts in 8 mins");
    });
  });

  describe("when the start is later today", () => {
    it("should name today", () => {
      expect(relativeStart("2099-06-01T17:00:00Z", NOW, "UTC")).toBe("Today 5:00 PM");
    });
  });

  describe("when the start is tomorrow", () => {
    it("should name tomorrow", () => {
      expect(relativeStart("2099-06-02T01:00:00Z", NOW, "UTC")).toBe("Tomorrow 1:00 AM");
    });
  });

  describe("when the start is several days away", () => {
    it("should name the date", () => {
      expect(relativeStart("2099-06-05T01:00:00Z", NOW, "UTC")).toBe("Fri, Jun 5 1:00 AM");
    });
  });

  describe("when the start is already past", () => {
    it("should show the time alone", () => {
      expect(relativeStart("2099-06-01T11:00:00Z", NOW, "UTC")).toBe("11:00 AM");
    });
  });
});

describe("page URLs", () => {
  describe("when opening an event", () => {
    it("should keep the other parameters and replace the place", () => {
      expect(eventHref("e/1", "day=4&position=1,2")).toBe("/events?day=4&id=e%2F1");
    });
  });

  describe("when opening a Genesis City place", () => {
    it("should keep the other parameters and replace the event", () => {
      expect(placeHref({ coordinates: [1, 2] }, "id=e1&day=4")).toBe("/events?day=4&position=1%2C2");
    });
  });

  describe("when opening a World place", () => {
    it("should link the World by name", () => {
      expect(placeHref({ coordinates: [0, 0], realm: "brai.dcl.eth" })).toBe("/events?world=brai.dcl.eth");
    });
  });

  describe("when closing a detail", () => {
    describe("and other parameters are present", () => {
      it("should keep them", () => {
        expect(closedHref("id=e1&day=4")).toBe("/events?day=4");
      });
    });

    describe("and only the detail parameter is present", () => {
      it("should return the bare page", () => {
        expect(closedHref("world=brai.dcl.eth")).toBe("/events");
      });
    });
  });

  describe("when sharing an event", () => {
    describe("and it is live", () => {
      it("should share its jump page", () => {
        expect(eventPageShareUrl("https://example.org", { id: "e1", live: true })).toBe(
          "https://example.org/jump/events?id=e1",
        );
      });
    });

    describe("and it is later", () => {
      it("should share this page", () => {
        expect(eventPageShareUrl("https://example.org", { id: "e1", live: false })).toBe(
          "https://example.org/events?id=e1",
        );
      });
    });
  });

  describe("when following a legacy link", () => {
    describe("and it is the list path", () => {
      it("should go to the events page", () => {
        expect(legacyEventsHref(new URL("https://example.org/whats-on"))).toBe("/events");
      });
    });

    describe("and it is a detail path with a query", () => {
      it("should keep the query and turn the path into the event to open", () => {
        expect(legacyEventsHref(new URL("https://example.org/whats-on/x?env=zone"), "x")).toBe("/events?env=zone&id=x");
      });
    });
  });

  describe("when reading the day offset", () => {
    describe("and the day parameter is present", () => {
      it.each([
        ["4", 4],
        ["-1", 0],
        ["1.5", 0],
        ["abc", 0],
        ["365", 365],
        ["400", 365],
        ["", 0],
      ] as const)("should read %j as %i whole days, capped at 365", (value, expected) => {
        expect(readDayOffset(new URLSearchParams({ day: value }))).toBe(expected);
      });
    });

    describe("and the day parameter is missing", () => {
      it("should read 0", () => {
        expect(readDayOffset(new URLSearchParams())).toBe(0);
      });
    });
  });
});

describe("eventCreator", () => {
  describe("when the creator is a wallet", () => {
    it("should link the creator to their profile", () => {
      expect(eventCreator({ user: "0xAbCdEf0000000000000000000000000000000001", user_name: "Ana" })).toEqual({
        name: "Ana",
        href: "/profile/0xabcdef0000000000000000000000000000000001",
      });
    });
  });

  describe("when the creator is not a wallet and has no name", () => {
    it("should name it Unknown without a link", () => {
      expect(eventCreator({ user: "not-a-wallet", user_name: null })).toEqual({ name: "Unknown", href: undefined });
    });
  });
});

describe("scheduleLine", () => {
  let event: Event;

  describe("when the event recurs weekly", () => {
    beforeEach(() => {
      event = eventRecord({ recurrent: true, recurrent_frequency: "WEEKLY", recurrent_interval: 2 });
    });

    it("should prefix the range with Starting and name its cadence", () => {
      expect(scheduleLine(event, "UTC")).toMatchObject({
        label: "Starting Mon, Jun 1 · 6:00 PM – 8:00 PM",
        recurrence: "Every 2 weeks",
      });
    });
  });

  describe("when the event has no valid start", () => {
    beforeEach(() => {
      event = eventRecord({ start_at: null, next_start_at: "not a date" });
    });

    it("should have no schedule", () => {
      expect(scheduleLine(event, "UTC")).toBeUndefined();
    });
  });

  describe("when the event recurs hourly", () => {
    it.each([[1, "Every hour"], [3, "Every 3 hours"]] as const)("should label interval %i as %s", (interval, label) => {
      expect(scheduleLine(eventRecord({ recurrent: true, recurrent_frequency: "HOURLY", recurrent_interval: interval }), "UTC")?.recurrence).toBe(label);
    });
  });
});

describe("creator faces", () => {
  const wallet = "0xabcdef0000000000000000000000000000000001";
  let profiles: NonNullable<Parameters<typeof eventCreator>[1]>;

  beforeEach(() => {
    profiles = { [wallet]: { name: "Ana", avatarUrl: "/profile-images/entities/x/face.png" } };
  });

  describe("when a profile read found the event creator's face", () => {
    it("should show the face", () => {
      expect(eventCreator({ user: wallet.toUpperCase().replace("0X", "0x"), user_name: "Ana" }, profiles).avatarUrl).toBe(
        "/profile-images/entities/x/face.png",
      );
    });
  });

  describe("when no profile was found for the event creator", () => {
    it("should have no face, so the initial shows", () => {
      expect(eventCreator({ user: wallet, user_name: "Ana" }, {}).avatarUrl).toBeUndefined();
    });
  });

  describe("when the place belongs to the Foundation", () => {
    let creator: ReturnType<typeof placeCreator>;

    beforeEach(() => {
      creator = placeCreator({ owner: wallet, contact_name: "Decentraland Foundation" }, profiles);
    });

    it("should keep the Foundation logo", () => {
      expect(creator).toMatchObject({ foundation: true });
    });

    it("should not use a wallet face", () => {
      expect(creator.avatarUrl).toBeUndefined();
    });
  });

  describe("when collecting creator wallets from events and places", () => {
    it("should keep only wallets", () => {
      expect(creatorWallets([{ user: wallet }, { user: "someone" }, { owner: null }, { owner: wallet }])).toEqual([
        wallet,
        wallet,
      ]);
    });
  });
});

describe("when the Events locale is Spanish", () => {
  it("should translate Today in the calendar", () => {
    expect(dayLabel(new Date(NOW), 0, "es")).toBe("Hoy");
  });

  it("should format a future start in the selected language and time convention", () => {
    expect(relativeStart("2099-06-01T18:00:00Z", NOW, "UTC", "es")).toBe("Hoy 18:00");
  });
});
