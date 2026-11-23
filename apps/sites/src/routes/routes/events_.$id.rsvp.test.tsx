import { describe, expect, it } from "vitest";

import { loader } from "./events_.$id.rsvp";

function target(id: string | undefined, search = ""): { status: number; location: string | null } {
  const request = new Request(`https://decentraland.example/events/${encodeURIComponent(id ?? "")}/rsvp${search}`);
  const response = loader({ request, params: { id } });
  return { status: response.status, location: response.headers.get("Location") };
}

describe("when an old RSVP link opens", () => {
  it("should open that event's detail on the Events page", () => {
    expect(target("e5923218-67fe-43b6-897e-c431df11ac54")).toEqual({
      status: 302,
      location: "/events?id=e5923218-67fe-43b6-897e-c431df11ac54",
    });
  });

  it("should encode an id that is not URL-safe", () => {
    expect(target("a b&c").location).toBe("/events?id=a+b%26c");
  });

  it("should keep the link's query, such as its campaign tags", () => {
    expect(target("ev-1", "?utm_source=mail&utm_campaign=launch").location).toBe("/events?utm_source=mail&utm_campaign=launch&id=ev-1");
  });

  it("should drop the old RSVP page's step", () => {
    expect(target("ev-1", "?step=confirm&utm_source=mail").location).toBe("/events?utm_source=mail&id=ev-1");
  });

  it("should let the path's id win over an id in the query", () => {
    expect(target("ev-1", "?id=other").location).toBe("/events?id=ev-1");
  });

  describe("and the id is blank", () => {
    it("should open the Events page", () => {
      expect(target("  ").location).toBe("/events");
      expect(target(undefined).location).toBe("/events");
    });

    it("should still keep the link's query", () => {
      expect(target(undefined, "?utm_source=mail&step=1").location).toBe("/events?utm_source=mail");
    });
  });
});
