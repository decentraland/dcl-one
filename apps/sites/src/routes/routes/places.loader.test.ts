import type { DehydratedState } from "@tanstack/react-query";
import { afterEach, describe, expect, it, vi } from "vitest";

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
  const result = (await loader({ request, params: {}, context: {} } as never)) as unknown as { data: Payload; init: ResponseInit | null };
  return { payload: result.data, status: result.init?.status ?? 200 };
}

function keys(state: DehydratedState) {
  return state.queries.map((query) => query.queryKey);
}

afterEach(() => vi.resetAllMocks());

describe("when the /places document loads", () => {
  it("should dehydrate the first Explore page and Live Now", async () => {
    vi.mocked(fetchDestinations).mockResolvedValue({ data: [destination()], total: 1, received: 1, copy: null });
    const { payload, status } = await run("http://localhost/places");
    expect(status).toBe(200);
    expect(keys(payload.dehydratedState)).toEqual(expect.arrayContaining([["places", "browse", "all", ""], ["places", "live"]]));
    expect(payload.platform.kind).toBe("desktop");
  });

  it("should pass the category and search of the URL to the feed", async () => {
    vi.mocked(fetchDestinations).mockResolvedValue({ data: [], total: 0, received: 0, copy: null });
    const { payload } = await run("http://localhost/places?category=Featured&search=plaza");
    expect(fetchDestinations).toHaveBeenCalledWith(expect.objectContaining({ only_highlighted: true, search: "plaza", limit: 48 }), expect.anything());
    // A search collapses Live Now, so its feed is not read.
    expect(fetchDestinations).toHaveBeenCalledTimes(1);
    expect(keys(payload.dehydratedState)).toEqual([["places", "browse", "featured", "plaza"]]);
  });

  it("should answer 503 and dehydrate no failed query when the catalogue is unavailable", async () => {
    vi.mocked(fetchDestinations).mockRejectedValue(new Error("Catalyst returned 502"));
    const { payload, status } = await run("http://localhost/places");
    expect(status).toBe(503);
    expect(payload.unavailable).toBe(true);
    expect(payload.dehydratedState.queries.filter((query) => query.state.status !== "success")).toEqual([]);
  });

  it("should still render with Live Now missing when only its read fails", async () => {
    vi.mocked(fetchDestinations).mockImplementation(async (params) => {
      if (params?.limit === 40) throw new Error("timeout");
      return { data: [destination()], total: 1, received: 1, copy: null };
    });
    const { status, payload } = await run("http://localhost/places");
    expect(status).toBe(200);
    expect(keys(payload.dehydratedState)).toEqual([["places", "browse", "all", ""]]);
  });

  it("should not read the browse feed for a member section", async () => {
    vi.mocked(fetchDestinations).mockResolvedValue({ data: [], total: 0, received: 0, copy: null });
    const { status } = await run("http://localhost/places?tab=favourites");
    expect(status).toBe(200);
    expect(fetchDestinations).toHaveBeenCalledTimes(1);
    expect(fetchDestinations).toHaveBeenCalledWith({ limit: 40 }, expect.anything());
  });

  it("should detect a phone from the user agent", async () => {
    vi.mocked(fetchDestinations).mockResolvedValue({ data: [], total: 0, received: 0, copy: null });
    const { payload } = await run("http://localhost/places", "Mozilla/5.0 (Linux; Android 14)");
    expect(payload.platform).toEqual({ kind: "mobile", os: "android" });
  });
});

describe("when the visitor disconnects during the document read", () => {
  it("should abort the upstream reads and not report an outage", async () => {
    const signals: AbortSignal[] = [];
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
    const pending = loader({ request, params: {}, context: {} } as never);
    await vi.waitFor(() => expect(signals).toHaveLength(2));
    controller.abort();
    const result = (await pending) as unknown as { data: Payload; init: ResponseInit | null };
    expect(signals.every((signal) => signal.aborted)).toBe(true);
    expect(result.data.unavailable).toBe(false);
  });
});

describe("when navigating to /places in the browser", () => {
  it("should hand no data from the client loader", async () => {
    await expect(clientLoader()).resolves.toBeNull();
  });
});
