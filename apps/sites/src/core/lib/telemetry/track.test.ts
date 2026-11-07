import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  DEFAULT_SEGMENT_TRACK_URL,
  buildSegmentBody,
  buildSegmentPayload,
  segmentTrackUrl,
  track,
  type TrackContext,
} from "./track";

const CTX: TrackContext = {
  sid: "sid-123",
  story: "homepage-hero",
  variant: "B",
};

const SEGMENT_BODY = {
  type: "track",
  event: "cta_clicked",
  anonymousId: "sid-123",
  properties: {
    surface: "hero",
    story: "homepage-hero",
    variant: "B",
  },
};

const trackAny = track as unknown as (
  event: string,
  props: Record<string, unknown>,
  ctx: TrackContext,
) => void;

describe("track (server path, fetch mocked)", () => {
  const fetchMock = vi.fn(() => Promise.resolve(new Response(null, { status: 200 })));

  beforeEach(() => {
    vi.stubGlobal("fetch", fetchMock);
    fetchMock.mockClear();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    delete process.env.SEGMENT_WRITE_KEY;
    delete process.env.SEGMENT_API_HOST;
  });

  it("preserves an event variant when the context does not override it", () => {
    expect(buildSegmentBody("cl_open_screen_shown", { variant: "base" }, { sid: "test" }).properties.variant).toBe("base");
    expect(buildSegmentBody("cl_open_screen_shown", { variant: "base" }, { sid: "test", variant: "genesis" }).properties.variant).toBe("genesis");
  });

  it("wraps the body in Segment's HTTP API envelope: the public write key, a message id, timestamps and the library context", () => {
    const now = new Date("2026-11-07T12:00:00.000Z");
    const payload = buildSegmentPayload(buildSegmentBody("cta_clicked", { surface: "hero" }, CTX), "key-1", now);
    expect(payload).toMatchObject({
      ...SEGMENT_BODY,
      writeKey: "key-1",
      integrations: {},
      timestamp: "2026-11-07T12:00:00.000Z",
      sentAt: "2026-11-07T12:00:00.000Z",
      context: { library: { name: "dcl-sites-beacon", version: "1.0.0" } },
    });
    expect(payload.messageId).toMatch(/^dcl-sites-beacon-/);
    expect(payload.context.direct).toBeUndefined();
  });

  it("POSTs the Segment body to the track endpoint without an Authorization header", async () => {
    expect(buildSegmentBody("cta_clicked", { surface: "hero" }, CTX)).toEqual(SEGMENT_BODY);
    process.env.SEGMENT_WRITE_KEY = "key-1";
    trackAny("cta_clicked", { surface: "hero" }, CTX);
    await Promise.resolve();
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe(DEFAULT_SEGMENT_TRACK_URL);
    expect(init.method).toBe("POST");
    const headers = init.headers as Record<string, string>;
    expect(headers.Authorization).toBeUndefined();
    expect(headers["Content-Type"]).toBe("application/json");
    const sent = JSON.parse(init.body as string);
    expect(sent).toMatchObject({ ...SEGMENT_BODY, writeKey: "key-1" });
    expect(typeof sent.messageId).toBe("string");
  });

  it("posts to the first-party proxy when SEGMENT_API_HOST is set, with or without a protocol or trailing slash", async () => {
    process.env.SEGMENT_WRITE_KEY = "key-1";
    process.env.SEGMENT_API_HOST = "api.e.example.org/v1/";
    expect(segmentTrackUrl()).toBe("https://api.e.example.org/v1/track");
    process.env.SEGMENT_API_HOST = "https://api.e.example.org/v1";
    expect(segmentTrackUrl()).toBe("https://api.e.example.org/v1/track");
    trackAny("evt", {}, CTX);
    await Promise.resolve();
    const [url] = fetchMock.mock.calls[0] as unknown as [string];
    expect(url).toBe("https://api.e.example.org/v1/track");
  });

  it("never throws: an unset write key skips fetch, a throwing fetch is swallowed, no ctx env is a no-op", async () => {
    expect(process.env.SEGMENT_WRITE_KEY).toBeUndefined();
    expect(() => trackAny("evt", { a: 1 }, CTX)).not.toThrow();
    expect(() => trackAny("evt", { x: 1 }, { sid: "anon" })).not.toThrow();
    await Promise.resolve();
    expect(fetchMock).not.toHaveBeenCalled();
    process.env.SEGMENT_WRITE_KEY = "key-1";
    fetchMock.mockImplementationOnce(() => {
      throw new Error("network down");
    });
    expect(() => trackAny("evt", {}, CTX)).not.toThrow();
    await Promise.resolve();
  });
});
