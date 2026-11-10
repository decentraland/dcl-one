// @vitest-environment jsdom
// The search box's data (R8-8 C1 addendum): its own key, never the search page's; 5 hits at skip 0;
// enabled only for a trimmed query of 3 characters or more; hits carry the highlight markup.
import "../locale/test-setup";
import type { ReactNode } from "react";
import { act, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { blogKeys } from "@/data/lib/blog/keys";
import { createFixtureBlogSource } from "@/data/lib/blog/fixture-source";
import { SEARCH_PAGE, SUGGEST_LIMIT } from "@/data/lib/blog/queries";
import type { BlogSource } from "@/data/lib/blog/source";

let source: BlogSource;
vi.mock("@/data/lib/blog/current-source", () => ({
  currentBlogSource: async () => source,
}));

import { searchQuery } from "./BlogShell";
import {
  SUGGEST_DEBOUNCE_MS,
  suggestQuery,
  useBlogSearchBox,
} from "./search-box";

beforeEach(() => {
  source = createFixtureBlogSource();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("suggestQuery", () => {
  it("keeps its own key, apart from the search page's for the same q", () => {
    expect(suggestQuery("wearables").queryKey).toEqual(
      blogKeys.suggest("wearables"),
    );
    expect(suggestQuery("wearables").queryKey).not.toEqual(
      searchQuery("wearables").queryKey,
    );
  });

  it("asks for 5 hits at skip 0 (and the search page for 10, so the two never share an answer)", async () => {
    const searchPosts = vi.spyOn(source, "searchPosts");
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    await client.fetchQuery(suggestQuery("decentraland"));
    expect(searchPosts).toHaveBeenCalledWith(
      "decentraland",
      { skip: 0, limit: SUGGEST_LIMIT },
      expect.any(AbortSignal),
    );
    expect([SUGGEST_LIMIT, SEARCH_PAGE.limit]).toEqual([5, 10]);
    // The page answer lands only under the suggest key.
    expect(client.getQueryData(blogKeys.suggest("decentraland"))).toBeDefined();
    expect(
      client.getQueryData(blogKeys.search("decentraland")),
    ).toBeUndefined();
  });

  it.each([
    ["ab", false],
    [" ab ", false],
    ["abc", true],
    ["  abc", true],
  ])("is enabled for %j: %s", (q, enabled) => {
    expect(suggestQuery(q).enabled).toBe(enabled);
  });
});

describe("useBlogSearchBox", () => {
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      {children}
    </QueryClientProvider>
  );

  it("searches the debounced value, showing Searching... until the hits arrive", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const searchPosts = vi.spyOn(source, "searchPosts");
    const { result } = renderHook(() => useBlogSearchBox(), { wrapper });
    act(() => result.current.onValueChange("decentraland"));
    expect(result.current.status).toBe("loading");
    expect(searchPosts).not.toHaveBeenCalled();
    await act(async () => {
      vi.advanceTimersByTime(SUGGEST_DEBOUNCE_MS);
    });
    await waitFor(() => expect(result.current.status).toBe("ready"));
    expect(searchPosts).toHaveBeenCalledTimes(1);
    expect(result.current.hits.length).toBeGreaterThan(0);
    // The sample has no _highlight, so each hit falls back to its stored fields.
    expect(result.current.hits[0]!.highlightedTitle).toBeTypeOf("string");
  });

  it("reports an upstream failure as an error", async () => {
    source = {
      ...createFixtureBlogSource(),
      searchPosts: async () => Promise.reject(new Error("down")),
    };
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const { result } = renderHook(() => useBlogSearchBox(), { wrapper });
    act(() => result.current.onValueChange("decentraland"));
    await act(async () => {
      vi.advanceTimersByTime(SUGGEST_DEBOUNCE_MS);
    });
    await waitFor(() => expect(result.current.status).toBe("error"));
  });
});
