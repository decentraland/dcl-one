import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  buildSegmentBody,
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
    delete process.env.TELEMETRY_URL;
  });

  it("preserves an event variant when the context does not override it", () => {
    expect(buildSegmentBody("cl_open_screen_shown", { variant: "base" }, { sid: "test" }).properties.variant).toBe("base");
    expect(buildSegmentBody("cl_open_screen_shown", { variant: "base" }, { sid: "test", variant: "genesis" }).properties.variant).toBe("genesis");
  });

  it("POSTs the exact Segment body to {TELEMETRY_URL}/v1/track with Basic dcl-sites auth, trimming a trailing slash", async () => {
    expect(buildSegmentBody("cta_clicked", { surface: "hero" }, CTX)).toEqual(SEGMENT_BODY);
    process.env.TELEMETRY_URL = "https://telemetry.example.com";
    trackAny("cta_clicked", { surface: "hero" }, CTX);
    await Promise.resolve();
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe("https://telemetry.example.com/v1/track");
    expect(init.method).toBe("POST");
    const headers = init.headers as Record<string, string>;
    expect(headers.Authorization).toBe("Basic dcl-sites");
    expect(headers["Content-Type"]).toBe("application/json");
    expect(JSON.parse(init.body as string)).toEqual(SEGMENT_BODY);

    process.env.TELEMETRY_URL = "https://telemetry.example.com/";
    trackAny("evt", {}, CTX);
    await Promise.resolve();
    const [second] = fetchMock.mock.calls[1] as unknown as [string];
    expect(second).toBe("https://telemetry.example.com/v1/track");
  });

  it("never throws: unset TELEMETRY_URL skips fetch, a throwing fetch is swallowed, no ctx env is a no-op", async () => {
    expect(process.env.TELEMETRY_URL).toBeUndefined();
    expect(() => trackAny("evt", { a: 1 }, CTX)).not.toThrow();
    expect(() => trackAny("evt", { x: 1 }, { sid: "anon" })).not.toThrow();
    await Promise.resolve();
    expect(fetchMock).not.toHaveBeenCalled();
    process.env.TELEMETRY_URL = "https://telemetry.example.com";
    fetchMock.mockImplementationOnce(() => {
      throw new Error("network down");
    });
    expect(() => trackAny("evt", {}, CTX)).not.toThrow();
    await Promise.resolve();
  });

});
