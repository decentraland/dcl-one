import type { DehydratedState } from "@tanstack/react-query";
import { RouterContextProvider } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import { fetchCommunitiesPage } from "@/data/lib/catalyst/social/communities-list";

import { clientLoader, loader } from "./places_.communities";

vi.mock("@/data/lib/catalyst/social/communities-list", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/social/communities-list")>()),
  fetchCommunitiesPage: vi.fn(),
}));

type Payload = { sid: string; dehydratedState: DehydratedState; unavailable: boolean };

async function run(url: string, request = new Request(url)) {
  const result = (await loader({ request, params: {}, context: new RouterContextProvider() } as never)) as unknown as {
    data: Payload;
    init: ResponseInit | null;
  };
  return { payload: result.data, status: result.init?.status ?? 200 };
}

const community = { id: "c1", name: "Builders", description: null, ownerName: null, privacy: "public" as const, membersCount: 3, thumbnailUrl: null };

afterEach(() => vi.resetAllMocks());

describe("when the /places/communities document loads", () => {
  it("should dehydrate the first page of the list", async () => {
    vi.mocked(fetchCommunitiesPage).mockResolvedValue({ rows: [community], received: 1, total: 1 });
    const { payload, status } = await run("http://localhost/places/communities");
    expect(status).toBe(200);
    expect(payload.unavailable).toBe(false);
    expect(payload.dehydratedState.queries.map((query) => query.queryKey)).toEqual([["communities", "browse", ""]]);
    expect(fetchCommunitiesPage).toHaveBeenCalledWith({ offset: 0, search: "" }, expect.anything());
  });

  it("should read the URL's search", async () => {
    vi.mocked(fetchCommunitiesPage).mockResolvedValue({ rows: [], received: 0, total: 0 });
    const { payload } = await run("http://localhost/places/communities?search=%20guild%20");
    expect(fetchCommunitiesPage).toHaveBeenCalledWith({ offset: 0, search: "guild" }, expect.anything());
    expect(payload.dehydratedState.queries.map((query) => query.queryKey)).toEqual([["communities", "browse", "guild"]]);
  });

  it("should read the whole list for a search shorter than three characters", async () => {
    vi.mocked(fetchCommunitiesPage).mockResolvedValue({ rows: [], received: 0, total: 0 });
    await run("http://localhost/places/communities?search=ab");
    expect(fetchCommunitiesPage).toHaveBeenCalledWith({ offset: 0, search: "" }, expect.anything());
  });

  it("should answer 503 and dehydrate no failed query when the list is unavailable", async () => {
    vi.mocked(fetchCommunitiesPage).mockRejectedValue(new Error("Catalyst returned 502"));
    const { payload, status } = await run("http://localhost/places/communities");
    expect(status).toBe(503);
    expect(payload.unavailable).toBe(true);
    expect(payload.dehydratedState.queries.filter((query) => query.state.status !== "success")).toEqual([]);
  });
});

describe("when the visitor disconnects during the document read", () => {
  it("should abort the upstream read and not report an outage", async () => {
    const signals: AbortSignal[] = [];
    vi.mocked(fetchCommunitiesPage).mockImplementation(
      (_params, options) =>
        new Promise((_resolve, reject) => {
          const signal = options?.signal;
          if (signal) signals.push(signal);
          signal?.addEventListener("abort", () => reject(new Error("aborted")));
        }),
    );
    const controller = new AbortController();
    const pending = run("http://localhost/places/communities", new Request("http://localhost/places/communities", { signal: controller.signal }));
    await vi.waitFor(() => expect(signals).toHaveLength(1));
    controller.abort();
    const { payload, status } = await pending;
    expect(signals[0]!.aborted).toBe(true);
    expect(status).toBe(200);
    expect(payload.unavailable).toBe(false);
  });
});

describe("when the page is reached by a client navigation", () => {
  it("should leave the read to the browser", async () => {
    await expect(clientLoader()).resolves.toBeNull();
  });
});
