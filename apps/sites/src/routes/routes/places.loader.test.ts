import type { DehydratedState } from "@tanstack/react-query";
import { RouterContextProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { destination } from "@/data/lib/catalyst/places/destination.fixture";
import { fetchDestinations } from "@/data/lib/catalyst/places/destinations";

import { clientLoader, loader } from "./places";

vi.mock("@/data/lib/catalyst/places/destinations", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/places/destinations")>()),
  fetchDestinations: vi.fn(),
}));

type Payload = { sid: string; platform: { kind: string }; dehydratedState: DehydratedState; unavailable: boolean };

async function run(url: string, userAgent = "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)") {
  const request = new Request(url, { headers: { "user-agent": userAgent } });
  const result = (await loader({ request, params: {}, context: new RouterContextProvider() } as never)) as unknown as { data: Payload; init: ResponseInit | null };
  return { payload: result.data, status: result.init?.status ?? 200 };
}

function keys(state: DehydratedState) {
  return state.queries.map((query) => query.queryKey);
}

const EMPTY = { data: [], total: 0, received: 0, copy: null };

describe("when the /places document loads", () => {
  let result: Awaited<ReturnType<typeof run>>;

  afterEach(() => vi.resetAllMocks());

  describe("and the catalogue answers", () => {
    beforeEach(async () => {
      vi.mocked(fetchDestinations).mockResolvedValue({ data: [destination()], total: 1, received: 1, copy: null });
      result = await run("http://localhost/places");
    });

    it("should answer 200", () => {
      expect(result.status).toBe(200);
    });

    it("should dehydrate the first Explore page and Live Now", () => {
      expect(keys(result.payload.dehydratedState)).toEqual(expect.arrayContaining([["places", "browse", "all", ""], ["places", "live"]]));
    });

    it("should detect a desktop from the user agent", () => {
      expect(result.payload.platform.kind).toBe("desktop");
    });
  });

  describe("and the URL carries a category and a search", () => {
    beforeEach(async () => {
      vi.mocked(fetchDestinations).mockResolvedValue(EMPTY);
      result = await run("http://localhost/places?category=Featured&search=plaza");
    });

    it("should pass the category and search to the feed", () => {
      expect(fetchDestinations).toHaveBeenCalledWith(expect.objectContaining({ only_highlighted: true, search: "plaza", limit: 48 }), expect.anything());
    });

    it("should not read the Live Now feed, which a search collapses", () => {
      expect(fetchDestinations).toHaveBeenCalledTimes(1);
    });

    it("should dehydrate only the searched feed", () => {
      expect(keys(result.payload.dehydratedState)).toEqual([["places", "browse", "featured", "plaza"]]);
    });
  });

  describe("and the catalogue is unavailable", () => {
    beforeEach(async () => {
      vi.mocked(fetchDestinations).mockRejectedValue(new Error("Catalyst returned 502"));
      result = await run("http://localhost/places");
    });

    it("should answer 503", () => {
      expect(result.status).toBe(503);
    });

    it("should flag the page unavailable", () => {
      expect(result.payload.unavailable).toBe(true);
    });

    it("should dehydrate no failed query", () => {
      expect(result.payload.dehydratedState.queries.filter((query) => query.state.status !== "success")).toEqual([]);
    });
  });

  describe("and only the Live Now read fails", () => {
    beforeEach(async () => {
      vi.mocked(fetchDestinations).mockImplementation(async (params) => {
        if (params?.limit === 40) throw new Error("timeout");
        return { data: [destination()], total: 1, received: 1, copy: null };
      });
      result = await run("http://localhost/places");
    });

    it("should still answer 200", () => {
      expect(result.status).toBe(200);
    });

    it("should render with Live Now missing", () => {
      expect(keys(result.payload.dehydratedState)).toEqual([["places", "browse", "all", ""]]);
    });
  });

  describe("and the URL opens a member section", () => {
    beforeEach(async () => {
      vi.mocked(fetchDestinations).mockResolvedValue(EMPTY);
      result = await run("http://localhost/places?tab=favourites");
    });

    it("should answer 200", () => {
      expect(result.status).toBe(200);
    });

    it("should not read the browse feed, only Live Now", () => {
      expect(fetchDestinations).toHaveBeenCalledTimes(1);
      expect(fetchDestinations).toHaveBeenCalledWith({ limit: 40, offset: 0 }, expect.anything());
    });
  });

  describe("and the user agent is a phone", () => {
    beforeEach(async () => {
      vi.mocked(fetchDestinations).mockResolvedValue(EMPTY);
      result = await run("http://localhost/places", "Mozilla/5.0 (Linux; Android 14)");
    });

    it("should detect a phone", () => {
      expect(result.payload.platform).toEqual({ kind: "mobile", os: "android" });
    });
  });
});

describe("when the visitor disconnects during the document read", () => {
  let signals: AbortSignal[];
  let result: { data: Payload; init: ResponseInit | null };

  beforeEach(async () => {
    signals = [];
    vi.mocked(fetchDestinations).mockImplementation(
      (_params, options) =>
        new Promise((_resolve, reject) => {
          const signal = options?.signal;
          if (signal) signals.push(signal);
          signal?.addEventListener("abort", () => reject(new Error("aborted")));
        }),
    );
    const controller = new AbortController();
    const request = new Request("http://localhost/places", { signal: controller.signal });
    const pending = loader({ request, params: {}, context: new RouterContextProvider() } as never);
    await vi.waitFor(() => expect(signals).toHaveLength(2));
    controller.abort();
    result = (await pending) as unknown as { data: Payload; init: ResponseInit | null };
  });

  afterEach(() => vi.resetAllMocks());

  it("should abort the upstream reads", () => {
    expect(signals.every((signal) => signal.aborted)).toBe(true);
  });

  it("should not report an outage", () => {
    expect(result.data.unavailable).toBe(false);
  });
});

describe("when navigating to /places in the browser", () => {
  it("should hand no data from the client loader", async () => {
    await expect(clientLoader()).resolves.toBeNull();
  });
});
