import { RouterContextProvider } from "react-router";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { CatalystError } from "@/data/lib/catalyst/client";
import { fetchEvent, fetchEvents } from "@/data/lib/catalyst/places/events";
import { fetchMostActivePlaces } from "@/data/lib/catalyst/places/index";
import { loadJumpPlace } from "@/data/lib/catalyst/jump/place-lookup";
import { eventsPageKeys } from "@/data/lib/catalyst/places/events-page";
import { eventRecord, placeRecord } from "@/features/events/fixtures";
import { requestSessionContext } from "@/core/lib/telemetry/session";
import { headers, loader } from "./events";

vi.mock("@/data/lib/catalyst/places/events", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/places/events")>()),
  fetchEvents: vi.fn(),
  fetchEvent: vi.fn(),
}));
vi.mock("@/data/lib/catalyst/places/index", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/places/index")>()),
  fetchMostActivePlaces: vi.fn(),
}));
vi.mock("@/data/lib/catalyst/jump/place-lookup", () => ({
  loadJumpPlace: vi.fn(),
}));

type Payload = {
  tab: "my" | null;
  unavailable: boolean;
  dehydratedState: { queries: { queryKey: readonly unknown[] }[] } | null;
};

async function load(path: string, userAgent = "Mozilla/5.0 (Macintosh)") {
  const result = (await loader({
    request: new Request(`https://example.org${path}`, {
      headers: { "user-agent": userAgent },
    }),
    context: new RouterContextProvider(),
  } as never)) as unknown as {
    data: Payload & { platform: { kind: string } };
    init: { status?: number } | null;
  };
  return { data: result.data, status: result.init?.status ?? 200 };
}

const keys = (data: Payload) =>
  data.dehydratedState?.queries.map((query) =>
    JSON.stringify(query.queryKey),
  ) ?? [];

beforeEach(() => {
  vi.mocked(fetchEvents)
    .mockReset()
    .mockResolvedValue({ data: [eventRecord()], total: 1 });
  vi.mocked(fetchEvent)
    .mockReset()
    .mockResolvedValue(eventRecord({ id: "deep" }));
  vi.mocked(fetchMostActivePlaces)
    .mockReset()
    .mockResolvedValue([placeRecord()]);
  vi.mocked(loadJumpPlace).mockReset().mockResolvedValue(null);
});

describe("the /events loader", () => {
  it("should hand the page its live, upcoming and calendar reads with a 200", async () => {
    const { data, status } = await load("/events");
    expect(status).toBe(200);
    expect(data.unavailable).toBe(false);
    expect(keys(data)).toEqual(
      expect.arrayContaining(
        [
          eventsPageKeys.live(),
          eventsPageKeys.upcoming(),
          eventsPageKeys.calendar(),
        ].map((key) => JSON.stringify(key)),
      ),
    );
  });

  it("should answer 503 when the calendar's upstream fails, and still hand over what did load", async () => {
    vi.mocked(fetchEvents).mockImplementation(async (params) => {
      if (params?.list === "active")
        throw new CatalystError("down", "/events/v1/events", 502);
      return { data: [eventRecord()], total: 1 };
    });
    const { data, status } = await load("/events");
    expect(status).toBe(503);
    expect(data.unavailable).toBe(true);
    expect(keys(data)).not.toContain(JSON.stringify(eventsPageKeys.calendar()));
    expect(keys(data)).toContain(JSON.stringify(eventsPageKeys.upcoming()));
  });

  it("should keep a 200 when only Live Now fails; the browser reads it again", async () => {
    vi.mocked(fetchMostActivePlaces).mockRejectedValue(
      new CatalystError("down", "/places", 502),
    );
    const { data, status } = await load("/events");
    expect(status).toBe(200);
    expect(keys(data)).not.toContain(JSON.stringify(eventsPageKeys.live()));
  });

  it("should load the event a deep link opens", async () => {
    const { data } = await load("/events?id=deep");
    expect(fetchEvent).toHaveBeenCalledWith("deep", expect.anything());
    expect(keys(data)).toContain(JSON.stringify(eventsPageKeys.detail("deep")));
  });

  it("should look a place link up by its parcel, and not when an event link wins", async () => {
    await load("/events?position=1,95");
    expect(loadJumpPlace).toHaveBeenCalledWith(
      { lookup: [1, 95], realm: undefined },
      expect.anything(),
    );
    vi.mocked(loadJumpPlace).mockClear();
    await load("/events?id=deep&position=1,95");
    expect(loadJumpPlace).not.toHaveBeenCalled();
  });

  it("should not read the public feed for My Events", async () => {
    const { data, status } = await load("/events?tab=my");
    expect(status).toBe(200);
    expect(data.tab).toBe("my");
    expect(fetchEvents).not.toHaveBeenCalled();
  });

  it("should detect a phone from the request, for Jump In's app launch", async () => {
    const { data } = await load(
      "/events",
      "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X)",
    );
    expect(data.platform.kind).toBe("mobile");
  });

  it("should use the session the request middleware already set", async () => {
    const context = new RouterContextProvider();
    context.set(requestSessionContext, {
      sid: "middleware-session",
      created: true,
    });
    const result = (await loader({
      request: new Request("https://example.org/events"),
      context,
    } as never)) as unknown as {
      data: { sid: string };
      init: { headers?: HeadersInit } | null;
    };

    expect(result.data.sid).toBe("middleware-session");
    expect(new Headers(result.init?.headers).getSetCookie()).toEqual([]);
  });

  it("should stop its upstream reads when the visitor leaves, and not answer 503", async () => {
    const seen: AbortSignal[] = [];
    vi.mocked(fetchEvents).mockImplementation(
      (_params, opts) =>
        new Promise((_resolve, reject) => {
          const signal = opts?.signal;
          if (signal) seen.push(signal);
          signal?.addEventListener("abort", () => reject(signal.reason), {
            once: true,
          });
        }),
    );
    const controller = new AbortController();
    const pending = loader({
      request: new Request("https://example.org/events", {
        signal: controller.signal,
      }),
      context: new RouterContextProvider(),
    } as never);
    await vi.waitFor(() => expect(seen.length).toBeGreaterThanOrEqual(3));
    controller.abort();
    await expect(pending).rejects.toBeDefined();
    expect(seen.every((signal) => signal.aborted)).toBe(true);
  });

  it("should keep the document, which carries the session id, out of shared caches", () => {
    expect(headers()).toMatchObject({ "Cache-Control": "private, no-cache" });
  });
});
