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
    expect(
      location(
        whatsOnDeeper({
          request: new Request("https://example.org/whats-on/a/b?tab=my"),
          params: { "*": "a/b" },
        } as never),
      ),
    ).toEqual([302, "/events?tab=my"]);
  });

  it("should keep a moderation link on its admin page under /events, with its query", () => {
    const request = new Request("https://example.org/whats-on/admin/pending-events?id=ev-1");
    expect(location(whatsOnDeeper({ request, params: { "*": "admin/pending-events" } } as never))).toEqual([
      302,
      "/events/admin/pending-events?id=ev-1",
    ]);
  });

  it("should open an edit-hangout link in the event editor, with its query", () => {
    const request = new Request("https://example.org/whats-on/edit-hangout/ev-1?openPreview");
    expect(location(whatsOnDeeper({ request, params: { "*": "edit-hangout/ev-1" } } as never))).toEqual([
      302,
      "/events/edit-event/ev-1?openPreview",
    ]);
  });

  it("should not treat a bare /whats-on/admin or an edit link without an id as a target", () => {
    for (const splat of ["admin", "edit-hangout/ev-1/extra"]) {
      const request = new Request(`https://example.org/whats-on/${splat}`);
      expect(location(whatsOnDeeper({ request, params: { "*": splat } } as never))).toEqual([302, "/events"]);
    }
  });

  it("should send the events site's /events/event?id= link to the same event on /events", () => {
    expect(
      location(eventsEvent({ request: new Request("https://example.org/events/event?id=abc&position=1,2") })),
    ).toEqual([302, "/events?id=abc&position=1%2C2"]);
  });
});
