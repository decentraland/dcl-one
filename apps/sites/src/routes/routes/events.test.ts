import { describe, expect, it } from "vitest";

import { loader as eventsEvent } from "./events_.event";
import { loader as whatsOn } from "./whats-on";
import { loader as whatsOnDetail } from "./whats-on_.$id";
import { loader as whatsOnDeeper } from "./whats-on_.$";

const location = (response: Response) => [response.status, response.headers.get("Location")];

describe("legacy events links", () => {
  it("should send /whats-on to /events with its query", () => {
    expect(
      location(whatsOn({ request: new Request("https://example.org/whats-on?search=music&category=art") })),
    ).toEqual([302, "/events?search=music&category=art"]);
  });

  it("should send a bare /whats-on to /events without an empty query", () => {
    expect(location(whatsOn({ request: new Request("https://example.org/whats-on") }))).toEqual([302, "/events"]);
  });

  it("should open a /whats-on/<id> detail as the event on /events, keeping the rest of the query", () => {
    const response = whatsOnDetail({
      request: new Request("https://example.org/whats-on/e%2F1?env=zone"),
      params: { id: "e/1" },
    } as never);
    expect(location(response)).toEqual([302, "/events?env=zone&id=e%2F1"]);
  });

  it("should send a deeper /whats-on path to /events with its query", () => {
    expect(location(whatsOnDeeper({ request: new Request("https://example.org/whats-on/a/b?tab=my") }))).toEqual([
      302,
      "/events?tab=my",
    ]);
  });

  it("should send the events site's /events/event?id= link to the same event on /events", () => {
    expect(
      location(eventsEvent({ request: new Request("https://example.org/events/event?id=abc&position=1,2") })),
    ).toEqual([302, "/events?id=abc&position=1%2C2"]);
  });
});
