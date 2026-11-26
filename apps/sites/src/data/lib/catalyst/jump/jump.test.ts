import { beforeEach, describe, expect, it } from "vitest";

import {
  eventHasEnded,
  eventShareUrl,
  genericPlaceCard,
  googleCalendarUrl,
  parsePosition,
  readJumpTarget,
  toEventCard,
  toPlaceCard,
  type JumpEventRecord,
  type JumpPlaceRecord,
} from "./jump";

const WALLET = "0x" + "ab".repeat(20);

const place = (over: Partial<JumpPlaceRecord> = {}): JumpPlaceRecord => ({
  title: "Plaza",
  description: "A square",
  image: "https://img.example/plaza.png",
  base_position: "10,20",
  owner: null,
  contact_name: null,
  user_count: 3,
  world: false,
  world_name: null,
  ...over,
});

const event = (over: Partial<JumpEventRecord> = {}): JumpEventRecord => ({
  id: "e1",
  name: "Party",
  image: null,
  description: "Come",
  next_start_at: "2030-01-01T10:00:00.000Z",
  next_finish_at: "2030-01-01T12:00:00.000Z",
  coordinates: [5, -3],
  user: WALLET,
  user_name: "host",
  total_attendees: 7,
  live: true,
  world: false,
  server: null,
  deleted_by_user: false,
  deleted_by_admin: false,
  ...over,
});

describe("parsePosition", () => {
  it.each([
    ["10,20", [10, 20]],
    ["10.20", [10, 20]],
    ["-3,-150", [-3, -150]],
  ])("should read %s as a coordinate pair", (value, coordinates) => {
    expect(parsePosition(value)).toEqual({ coordinates, isValid: true });
  });

  it.each(["", "10", "a,b", "1,2,3", ",", "1abc,2", "1,2x", "1000,0", "0,-1000", "3000000000,0", "1.5,2"])("should reject %j", (value) => {
    expect(parsePosition(value)).toEqual({ coordinates: [0, 0], isValid: false });
  });
});

describe("readJumpTarget", () => {
  describe("when the link carries nothing", () => {
    it("should default to Genesis City origin", () => {
      expect(readJumpTarget(new URLSearchParams())).toEqual({
        coordinates: [0, 0],
        validPosition: true,
        validRealm: true,
        lookup: [0, 0],
        validId: true,
        realm: undefined,
        id: null,
      });
    });

    it("should read no realm as a valid Genesis City realm", () => {
      expect(readJumpTarget(new URLSearchParams())).toMatchObject({ realm: undefined, validRealm: true });
    });
  });

  describe("when the realm is main", () => {
    describe("and the link carries a position", () => {
      it("should treat the realm as Genesis City", () => {
        expect(readJumpTarget(new URLSearchParams("realm=main&position=1,2")).realm).toBeUndefined();
      });
    });

    describe("and the link carries nothing else", () => {
      it("should read it as a valid Genesis City realm", () => {
        expect(readJumpTarget(new URLSearchParams("realm=main"))).toMatchObject({ realm: undefined, validRealm: true });
      });
    });
  });

  describe("when the realm is a World name", () => {
    it("should read it lowercased", () => {
      expect(readJumpTarget(new URLSearchParams("realm=Foo.DCL.eth"))).toMatchObject({ realm: "foo.dcl.eth", validRealm: true });
    });

    describe("and the link has no position", () => {
      it("should look up the World-level record", () => {
        expect(readJumpTarget(new URLSearchParams("realm=foo.dcl.eth")).lookup).toBeUndefined();
      });
    });
  });

  describe("when ?realm= is empty", () => {
    describe("and ?world= is given", () => {
      it("should fall back to ?world=", () => {
        expect(readJumpTarget(new URLSearchParams("realm=&world=Foo.dcl.eth")).realm).toBe("foo.dcl.eth");
      });
    });

    describe("and only a position is given", () => {
      it("should read the realm as Genesis City", () => {
        expect(readJumpTarget(new URLSearchParams("realm=&position=1,2"))).toMatchObject({ realm: undefined, validRealm: true });
      });
    });
  });

  describe("when the realm is not a World name", () => {
    it.each(["brai", "https://evil.example", "brai.dcl"])("should flag the realm %j as invalid", (value) => {
      expect(readJumpTarget(new URLSearchParams({ world: value }))).toMatchObject({ realm: undefined, validRealm: false });
    });
  });

  describe("when only ?world= is given", () => {
    it("should accept it as the realm alias", () => {
      expect(readJumpTarget(new URLSearchParams("world=foo.dcl.eth")).realm).toBe("foo.dcl.eth");
    });
  });

  describe("when the position is not a coordinate pair", () => {
    it("should flag it", () => {
      expect(readJumpTarget(new URLSearchParams("position=nope")).validPosition).toBe(false);
    });
  });

  describe("when the link carries an id", () => {
    it.each([["abc", false], ["d275788c-2e03-4163-b7aa-2d997f679936", true]])("should flag the id %j as valid=%s", (id, valid) => {
      expect(readJumpTarget(new URLSearchParams({ id })).validId).toBe(valid);
    });

    describe("and the id is blank", () => {
      it("should ignore it", () => {
        expect(readJumpTarget(new URLSearchParams("id=%20")).id).toBeNull();
      });
    });
  });
});

describe("toPlaceCard", () => {
  describe("when the scene has a contact and an owner", () => {
    it("should prefer the scene contact over the owner", () => {
      expect(toPlaceCard(place({ contact_name: "Studio", owner: WALLET }))).toMatchObject({
        creatorName: "Studio",
        creatorAddress: WALLET,
      });
    });
  });

  describe("when the owner is a wallet", () => {
    it("should never show it as the creator name", () => {
      expect(toPlaceCard(place({ owner: WALLET })).creatorName).toBe("Unknown");
    });
  });

  describe("when the owner is not a wallet", () => {
    it("should show it as a name without a profile address", () => {
      expect(toPlaceCard(place({ owner: "dexou by xyz" }))).toMatchObject({
        creatorName: "dexou by xyz",
        creatorAddress: undefined,
      });
    });
  });

  describe("when the place is a World", () => {
    it("should carry the World name as the realm", () => {
      expect(toPlaceCard(place({ world: true, world_name: "foo.dcl.eth" })).realm).toBe("foo.dcl.eth");
    });
  });
});

describe("genericPlaceCard", () => {
  describe("when the card is for a World", () => {
    it("should title it with the World name", () => {
      expect(genericPlaceCard([1, 2], "foo.dcl.eth")).toMatchObject({ title: "foo.dcl.eth", coordinates: [1, 2] });
    });
  });
});

describe("toEventCard", () => {
  describe("when a World event's server is not a World name", () => {
    it("should not travel to it", () => {
      expect(toEventCard(event({ world: true, server: "https://other.example" })).realm).toBeUndefined();
    });
  });

  describe("when a World event's server is a World name", () => {
    it("should travel to it", () => {
      expect(toEventCard(event({ world: true, server: "Foo.DCL.eth" })).realm).toBe("foo.dcl.eth");
    });
  });

  describe("when the event was deleted", () => {
    it("should never show it as live or ended", () => {
      expect(toEventCard(event({ deleted_by_admin: true })).event).toMatchObject({
        live: false,
        finishAt: null,
        deleted: true,
      });
    });
  });
});

describe("eventHasEnded", () => {
  const now = Date.parse("2030-01-01T11:00:00.000Z");

  describe("when the finish is ahead", () => {
    it("should be false", () => {
      expect(eventHasEnded("2030-01-01T12:00:00.000Z", now)).toBe(false);
    });
  });

  describe("when the finish has passed", () => {
    it("should be true", () => {
      expect(eventHasEnded("2030-01-01T10:30:00.000Z", now)).toBe(true);
    });
  });

  describe("when the finish is missing or invalid", () => {
    it.each([null, undefined, "not a date"])("should be false for %j", (finish) => {
      expect(eventHasEnded(finish, now)).toBe(false);
    });
  });
});

describe("eventShareUrl", () => {
  it("should link the event itself, with its parcel and realm, on /jump/events", () => {
    expect(eventShareUrl("https://site.example", "e1", { coordinates: [5, -3], realm: "foo.dcl.eth" })).toBe(
      "https://site.example/jump/events?id=e1&position=5%2C-3&realm=foo.dcl.eth",
    );
  });
});

describe("googleCalendarUrl", () => {
  describe("when the event has a valid start and finish", () => {
    let url: URL;

    beforeEach(() => {
      url = new URL(
        googleCalendarUrl(
          { title: "Party", description: "Come", startAt: "2030-01-01T10:00:00.000Z", finishAt: "2030-01-01T12:00:00.000Z" },
          "https://site.example/jump/events?position=5%2C-3",
        )!,
      );
    });

    it("should stamp the dates in UTC", () => {
      expect(url.searchParams.get("dates")).toBe("20300101T100000Z/20300101T120000Z");
    });

    it("should add the jump link to the details", () => {
      expect(url.searchParams.get("details")).toBe("Come\n\nJump In: https://site.example/jump/events?position=5%2C-3");
    });
  });

  describe.each([
    [null, "2030-01-01T12:00:00.000Z"],
    ["2030-01-01T10:00:00.000Z", null],
    ["bad", "2030-01-01T12:00:00.000Z"],
  ])("when the start or finish is not valid (%j, %j)", (startAt, finishAt) => {
    it("should return null", () => {
      expect(googleCalendarUrl({ title: "Party", startAt, finishAt }, "https://x")).toBeNull();
    });
  });
});
