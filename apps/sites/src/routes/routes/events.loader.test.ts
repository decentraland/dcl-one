import { RouterContextProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

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

describe("the /events loader", () => {
  let result: Awaited<ReturnType<typeof load>>;

  beforeEach(() => {
    vi.mocked(fetchEvents).mockResolvedValue({ data: [eventRecord()], total: 1 });
    vi.mocked(fetchEvent).mockResolvedValue(eventRecord({ id: "deep" }));
    vi.mocked(fetchMostActivePlaces).mockResolvedValue([placeRecord()]);
    vi.mocked(loadJumpPlace).mockResolvedValue(null);
  });

  afterEach(() => {
    vi.resetAllMocks();
  });

  describe("when every upstream answers", () => {
    beforeEach(async () => {
      result = await load("/events");
    });

    it("should answer 200", () => {
      expect(result.status).toBe(200);
    });

    it("should not flag the page unavailable", () => {
      expect(result.data.unavailable).toBe(false);
    });

    it("should hand the page its live, upcoming and calendar reads", () => {
      expect(keys(result.data)).toEqual(
        expect.arrayContaining(
          [
            eventsPageKeys.live(),
            eventsPageKeys.upcoming(),
            eventsPageKeys.calendar(),
          ].map((key) => JSON.stringify(key)),
        ),
      );
    });
  });

  describe("when the calendar's upstream fails", () => {
    beforeEach(async () => {
      vi.mocked(fetchEvents).mockImplementation(async (params) => {
        if (params?.list === "active")
          throw new CatalystError("down", "/events/v1/events", 502);
        return { data: [eventRecord()], total: 1 };
      });
      result = await load("/events");
    });

    it("should answer 503", () => {
      expect(result.status).toBe(503);
    });

    it("should flag the page unavailable", () => {
      expect(result.data.unavailable).toBe(true);
    });

    it("should not hand over the calendar", () => {
      expect(keys(result.data)).not.toContain(JSON.stringify(eventsPageKeys.calendar()));
    });

    it("should still hand over what did load", () => {
      expect(keys(result.data)).toContain(JSON.stringify(eventsPageKeys.upcoming()));
    });
  });

  describe("when only Live Now fails", () => {
    beforeEach(async () => {
      vi.mocked(fetchMostActivePlaces).mockRejectedValue(
        new CatalystError("down", "/places", 502),
      );
      result = await load("/events");
    });

    it("should keep a 200", () => {
      expect(result.status).toBe(200);
    });

    it("should leave Live Now for the browser to read again", () => {
      expect(keys(result.data)).not.toContain(JSON.stringify(eventsPageKeys.live()));
    });
  });

  describe("when a deep link opens an event", () => {
    beforeEach(async () => {
      result = await load("/events?id=deep");
    });

    it("should load the event", () => {
      expect(fetchEvent).toHaveBeenCalledWith("deep", expect.anything());
    });

    it("should hand the event to the page", () => {
      expect(keys(result.data)).toContain(JSON.stringify(eventsPageKeys.detail("deep")));
    });
  });

  describe("when a link carries a place position", () => {
    describe("and no event", () => {
      beforeEach(async () => {
        await load("/events?position=1,95");
      });

      it("should look the place up by its parcel", () => {
        expect(loadJumpPlace).toHaveBeenCalledWith(
          { lookup: [1, 95], realm: undefined },
          expect.anything(),
        );
      });
    });

    describe("and an event, which wins", () => {
      beforeEach(async () => {
        await load("/events?id=deep&position=1,95");
      });

      it("should not look the place up", () => {
        expect(loadJumpPlace).not.toHaveBeenCalled();
      });
    });
  });

  describe("when the visitor opens My Events", () => {
    beforeEach(async () => {
      result = await load("/events?tab=my");
    });

    it("should answer 200", () => {
      expect(result.status).toBe(200);
    });

    it("should hand the page the My Events tab", () => {
      expect(result.data.tab).toBe("my");
    });

    it("should render the public feed around it, since it is a section of the same page", () => {
      expect(fetchEvents).toHaveBeenCalled();
      expect(result.data.dehydratedState).not.toBeNull();
    });
  });

  describe("when the tab value is unknown", () => {
    beforeEach(async () => {
      result = await load("/events?tab=other");
    });

    it("should mark no tab", () => {
      expect(result.data.tab).toBeNull();
    });
  });

  describe("when the request comes from a phone", () => {
    beforeEach(async () => {
      result = await load(
        "/events",
        "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X)",
      );
    });

    it("should detect it, for Jump In's app launch", () => {
      expect(result.data.platform.kind).toBe("mobile");
    });
  });

  describe("when the request middleware already set the session", () => {
    let sessionResult: {
      data: { sid: string };
      init: { headers?: HeadersInit } | null;
    };

    beforeEach(async () => {
      const context = new RouterContextProvider();
      context.set(requestSessionContext, {
        sid: "middleware-session",
        created: true,
      });
      sessionResult = (await loader({
        request: new Request("https://example.org/events"),
        context,
      } as never)) as unknown as typeof sessionResult;
    });

    it("should use that session", () => {
      expect(sessionResult.data.sid).toBe("middleware-session");
    });

    it("should set no cookie", () => {
      expect(new Headers(sessionResult.init?.headers).getSetCookie()).toEqual([]);
    });
  });

  describe("when the visitor leaves during the read", () => {
    let seen: AbortSignal[];
    let controller: AbortController;
    let pending: Promise<unknown>;

    beforeEach(async () => {
      seen = [];
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
      controller = new AbortController();
      pending = loader({
        request: new Request("https://example.org/events", {
          signal: controller.signal,
        }),
        context: new RouterContextProvider(),
      } as never) as Promise<unknown>;
      await vi.waitFor(() => expect(seen.length).toBeGreaterThanOrEqual(3));
    });

    it("should stop its upstream reads and not answer 503", async () => {
      controller.abort();
      await expect(pending).rejects.toBeDefined();
      expect(seen.every((signal) => signal.aborted)).toBe(true);
    });
  });
});

describe("the /events headers", () => {
  it("should keep the document, which carries the session id, out of shared caches", () => {
    expect(headers()).toMatchObject({ "Cache-Control": "private, no-cache" });
  });
});
