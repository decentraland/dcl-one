import { describe, expect, it } from "vitest";

import { eventHasEnded, formatEventDate, parsePosition, readJumpTarget } from "./jump";

describe("jump links", () => {
  it("reads x,y and x.y as the same position", () => {
    expect(parsePosition("10,20")).toEqual({ coordinates: [10, 20], isValid: true });
    expect(parsePosition("10.20")).toEqual({ coordinates: [10, 20], isValid: true });
  });

  it("calls anything that is not two integers invalid", () => {
    expect(parsePosition("abc").isValid).toBe(false);
    expect(parsePosition("1,2,3").isValid).toBe(false);
    expect(parsePosition("x,1").isValid).toBe(false);
  });

  it("defaults to Genesis Plaza on the main realm", () => {
    expect(readJumpTarget(new URLSearchParams(""))).toEqual({ position: { coordinates: [0, 0], isValid: true }, placesPosition: [0, 0], realm: undefined, id: undefined });
    expect(readJumpTarget(new URLSearchParams("realm=main")).realm).toBeUndefined();
  });

  it("takes ?world= as ?realm= and asks a World without a position for the World itself", () => {
    const target = readJumpTarget(new URLSearchParams("world=foo.dcl.eth"));
    expect(target.realm).toBe("foo.dcl.eth");
    expect(target.placesPosition).toBeUndefined();
    expect(readJumpTarget(new URLSearchParams("world=foo.dcl.eth&position=1,2")).placesPosition).toEqual([1, 2]);
  });

  it("says an event has ended only after its finish", () => {
    const finish = "2026-11-10T17:30:00.000Z";
    expect(eventHasEnded(finish, Date.parse("2026-11-10T17:00:00Z"))).toBe(false);
    expect(eventHasEnded(finish, Date.parse("2026-11-10T18:00:00Z"))).toBe(true);
    expect(eventHasEnded(undefined)).toBe(false);
  });

  it("prints the start as decentraland.org does", () => {
    expect(formatEventDate("2026-11-10T16:00:00.000Z", "UTC")).toBe("Nov 10, 2026, 04:00 PM UTC");
  });
});
