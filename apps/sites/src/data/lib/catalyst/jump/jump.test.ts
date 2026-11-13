import { describe, expect, it } from "vitest";

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
  it("should default to Genesis City origin when the link carries nothing", () => {
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

  it("should treat the main realm as Genesis City", () => {
    expect(readJumpTarget(new URLSearchParams("realm=main&position=1,2")).realm).toBeUndefined();
  });

  it("should read only a World name as a realm, lowercased", () => {
    expect(readJumpTarget(new URLSearchParams("realm=Foo.DCL.eth"))).toMatchObject({ realm: "foo.dcl.eth", validRealm: true });
    expect(readJumpTarget(new URLSearchParams("realm=main"))).toMatchObject({ realm: undefined, validRealm: true });
    expect(readJumpTarget(new URLSearchParams())).toMatchObject({ realm: undefined, validRealm: true });
  });

  it("should fall back to ?world= when ?realm= is empty", () => {
    expect(readJumpTarget(new URLSearchParams("realm=&world=Foo.dcl.eth")).realm).toBe("foo.dcl.eth");
  });

  it("should read an empty realm as Genesis City", () => {
    expect(readJumpTarget(new URLSearchParams("realm=&position=1,2"))).toMatchObject({ realm: undefined, validRealm: true });
  });

  it.each(["brai", "https://evil.example", "brai.dcl"])("should flag the realm %j as invalid", (value) => {
    expect(readJumpTarget(new URLSearchParams({ world: value }))).toMatchObject({ realm: undefined, validRealm: false });
  });

  it("should accept ?world= as the realm alias", () => {
    expect(readJumpTarget(new URLSearchParams("world=foo.dcl.eth")).realm).toBe("foo.dcl.eth");
  });

  it("should look up the World-level record when a World link has no position", () => {
    expect(readJumpTarget(new URLSearchParams("realm=foo.dcl.eth")).lookup).toBeUndefined();
  });

  it("should flag a position that is not a coordinate pair", () => {
    expect(readJumpTarget(new URLSearchParams("position=nope")).validPosition).toBe(false);
  });

  it.each([["abc", false], ["d275788c-2e03-4163-b7aa-2d997f679936", true]])("should flag the id %j as valid=%s", (id, valid) => {
    expect(readJumpTarget(new URLSearchParams({ id })).validId).toBe(valid);
  });

  it("should ignore a blank id", () => {
    expect(readJumpTarget(new URLSearchParams("id=%20")).id).toBeNull();
  });
});

describe("toPlaceCard", () => {
  it("should prefer the scene contact over the owner", () => {
    expect(toPlaceCard(place({ contact_name: "Studio", owner: WALLET }))).toMatchObject({
      creatorName: "Studio",
      creatorAddress: WALLET,
    });
  });

  it("should never show a wallet owner as the creator name", () => {
    expect(toPlaceCard(place({ owner: WALLET })).creatorName).toBe("Unknown");
  });

  it("should show a non-wallet owner as a name without a profile address", () => {
    expect(toPlaceCard(place({ owner: "dexou by xyz" }))).toMatchObject({
      creatorName: "dexou by xyz",
      creatorAddress: undefined,
    });
  });

  it("should carry the World name as the realm", () => {
    expect(toPlaceCard(place({ world: true, world_name: "foo.dcl.eth" })).realm).toBe("foo.dcl.eth");
  });
});

describe("genericPlaceCard", () => {
  it("should title a World card with the World name", () => {
    expect(genericPlaceCard([1, 2], "foo.dcl.eth")).toMatchObject({ title: "foo.dcl.eth", coordinates: [1, 2] });
  });
});

describe("toEventCard", () => {
  it("should not travel to an event server that is not a World name", () => {
    expect(toEventCard(event({ world: true, server: "https://other.example" })).realm).toBeUndefined();
  });

  it("should travel to a World event's server", () => {
    expect(toEventCard(event({ world: true, server: "Foo.DCL.eth" })).realm).toBe("foo.dcl.eth");
  });

  it("should never show a deleted event as live or ended", () => {
    expect(toEventCard(event({ deleted_by_admin: true })).event).toMatchObject({
      live: false,
      finishAt: null,
      deleted: true,
    });
  });
});

describe("eventHasEnded", () => {
  const now = Date.parse("2030-01-01T11:00:00.000Z");

  it("should be false before the finish and true after it", () => {
    expect(eventHasEnded("2030-01-01T12:00:00.000Z", now)).toBe(false);
    expect(eventHasEnded("2030-01-01T10:30:00.000Z", now)).toBe(true);
  });

  it.each([null, undefined, "not a date"])("should be false for %j", (finish) => {
    expect(eventHasEnded(finish, now)).toBe(false);
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
  it("should build the template link with UTC stamps and the jump link", () => {
    const url = new URL(
      googleCalendarUrl(
        { title: "Party", description: "Come", startAt: "2030-01-01T10:00:00.000Z", finishAt: "2030-01-01T12:00:00.000Z" },
        "https://site.example/jump/events?position=5%2C-3",
      )!,
    );
    expect(url.searchParams.get("dates")).toBe("20300101T100000Z/20300101T120000Z");
    expect(url.searchParams.get("details")).toBe("Come\n\nJump In: https://site.example/jump/events?position=5%2C-3");
  });

  it.each([
    [null, "2030-01-01T12:00:00.000Z"],
    ["2030-01-01T10:00:00.000Z", null],
    ["bad", "2030-01-01T12:00:00.000Z"],
  ])("should return null without a valid start and finish (%j, %j)", (startAt, finishAt) => {
    expect(googleCalendarUrl({ title: "Party", startAt, finishAt }, "https://x")).toBeNull();
  });
});
