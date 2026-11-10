import {
  InfiniteQueryObserver,
  QueryClient,
  dehydrate,
  hydrate,
} from "@tanstack/react-query";
import { describe, expect, it } from "vitest";

import { blogKeys } from "./keys";
import {
  blogListQuery,
  type BlogListData,
  type FetchBlogPage,
} from "./queries";
import type { BlogPage, BlogPost } from "./types";

// A list of N posts the fake upstream pages through; each post is only an id here.
const corpus = (n: number) =>
  Array.from({ length: n }, (_, i) => ({ id: `p${i}` }) as unknown as BlogPost);

function upstream(posts: BlogPost[]) {
  const requests: { skip: number; limit: number }[] = [];
  const fetchPage: FetchBlogPage = async (_scope, param) => {
    requests.push(param);
    return {
      items: posts.slice(param.skip, param.skip + param.limit),
      total: posts.length,
      skip: param.skip,
      limit: param.limit,
    };
  };
  return { fetchPage, requests };
}

const client = () =>
  new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: 30_000 } },
  });
const key = blogKeys.list({ scope: "home" });
const ids = (data: { pages: BlogPage[] } | undefined) =>
  (data?.pages ?? []).flatMap((page) => page.items.map((item) => item.id));

describe("blogListQuery", () => {
  it("should page 7 first, then 6 at a time, and stop at the total", async () => {
    const qc = client();
    const { fetchPage, requests } = upstream(corpus(20));
    const observer = new InfiniteQueryObserver(
      qc,
      blogListQuery({ scope: "home" }, fetchPage),
    );
    const unsubscribe = observer.subscribe(() => undefined);
    await observer.refetch();
    await observer.fetchNextPage();
    await observer.fetchNextPage();
    const result = observer.getCurrentResult();

    expect(requests).toEqual([
      { skip: 0, limit: 7 },
      { skip: 7, limit: 6 },
      { skip: 13, limit: 6 },
    ]);
    expect(ids(result.data)).toHaveLength(19);
    await observer.fetchNextPage();
    expect(observer.getCurrentResult().hasNextPage).toBe(false);
    expect(ids(observer.getCurrentResult().data)).toHaveLength(20);
    unsubscribe();
  });
});

describe("loader dehydration into the host client", () => {
  it("should reuse the first page without fetching it a second time", async () => {
    const loader = client();
    const host = client();
    const source = upstream(corpus(20));
    const options = blogListQuery({ scope: "home" }, source.fetchPage);
    await loader.fetchInfiniteQuery(options);
    hydrate(host, dehydrate(loader));
    loader.clear();
    const observer = new InfiniteQueryObserver(host, options);
    const stop = observer.subscribe(() => undefined);
    expect(ids(observer.getCurrentResult().data)).toHaveLength(7);
    expect(source.requests).toEqual([{ skip: 0, limit: 7 }]);
    stop();
    host.clear();
  });
  it("should isolate two simultaneous loader clients", async () => {
    const first = client();
    const second = client();
    await Promise.all([
      first.fetchInfiniteQuery(
        blogListQuery({ scope: "home" }, upstream(corpus(20)).fetchPage),
      ),
      second.fetchInfiniteQuery(
        blogListQuery({ scope: "home" }, upstream(corpus(3)).fetchPage),
      ),
    ]);
    expect(ids(first.getQueryData(key))).toHaveLength(7);
    expect(ids(second.getQueryData(key))).toHaveLength(3);
    first.clear();
    second.clear();
  });
  it("should omit failed queries from the document state", async () => {
    const loader = client();
    const options = blogListQuery({ scope: "home" }, async () => {
      throw new Error("source unavailable");
    });
    await expect(loader.fetchInfiniteQuery(options)).rejects.toThrow(
      "source unavailable",
    );
    expect(dehydrate(loader).queries).toEqual([]);
    loader.clear();
  });
});

describe("client refetch consistency", () => {
  function upstreamOf(initial: BlogPost[], delay = 20) {
    let posts = initial;
    const requests: { skip: number; limit: number }[] = [];
    let aborted = 0;
    const fetchPage: FetchBlogPage = (_scope, param, signal) => {
      requests.push(param);
      return new Promise((resolve, reject) => {
        const onAbort = () => {
          aborted += 1;
          clearTimeout(timer);
          reject(signal.reason);
        };
        const timer = setTimeout(() => {
          signal.removeEventListener("abort", onAbort);
          resolve({
            items: posts.slice(param.skip, param.skip + param.limit),
            total: posts.length,
            ...param,
          });
        }, delay);
        signal.addEventListener("abort", onAbort);
      });
    };
    return {
      fetchPage,
      requests,
      aborted: () => aborted,
      set: (next: BlogPost[]) => (posts = next),
    };
  }
  const watch = (observer: {
    subscribe: (
      listener: (result: {
        isError: boolean;
        data?: { pages: BlogPage[] };
      }) => void,
    ) => () => void;
  }) => {
    const seen: { isError: boolean; ids: string[] }[] = [];
    const unsubscribe = observer.subscribe((result) =>
      seen.push({ isError: result.isError, ids: ids(result.data) }),
    );
    return { seen, unsubscribe };
  };

  // The R3 changed-tail case, as a client refetch: 3 pages cached, then positions 8 and 20 swap.
  it("should refetch three pages after the tail changed with every item exactly once and no mixed list", async () => {
    const qc = client();
    const before = corpus(25);
    const upstream = upstreamOf(before);
    const observer = new InfiniteQueryObserver(
      qc,
      blogListQuery({ scope: "home" }, upstream.fetchPage),
    );
    const { seen, unsubscribe } = watch(observer);
    await observer.refetch();
    await observer.fetchNextPage();
    await observer.fetchNextPage();
    const after = [...before];
    [after[8], after[20]] = [after[20]!, after[8]!];
    upstream.set(after);
    seen.length = 0;
    upstream.requests.length = 0;

    await observer.refetch();
    const final = ids(qc.getQueryData<BlogListData>(key));

    expect(upstream.requests.map((request) => request.skip)).toEqual([
      0, 7, 13,
    ]);
    expect(final).toEqual(after.slice(0, 19).map((post) => post.id));
    expect(new Set(final).size).toBe(final.length);
    for (const state of seen) {
      expect(state.isError).toBe(false);
      const old = state.ids.every((id, i) => id === before[i]!.id);
      const fresh = state.ids.every((id, i) => id === after[i]!.id);
      expect(old || fresh).toBe(true);
    }
    unsubscribe();
  });

  it("should end consistent when a refetch runs while an older next page is in flight", async () => {
    const qc = client();
    const before = corpus(25);
    const upstream = upstreamOf(before, 60);
    const observer = new InfiniteQueryObserver(
      qc,
      blogListQuery({ scope: "home" }, upstream.fetchPage),
    );
    const { seen, unsubscribe } = watch(observer);
    await observer.refetch();
    const nextPage = observer.fetchNextPage();
    await new Promise((resolve) => setTimeout(resolve, 10));
    const after = [{ id: "new" } as unknown as BlogPost, ...before];
    upstream.set(after);

    await qc.refetchQueries({ queryKey: key });
    await nextPage;
    const final = ids(qc.getQueryData<BlogListData>(key));

    expect(upstream.aborted()).toBe(1);
    expect(final).toEqual(after.slice(0, 7).map((post) => post.id));
    for (const state of seen) {
      expect(state.isError).toBe(false);
      const old = state.ids.every((id, i) => id === before[i]!.id);
      const fresh = state.ids.every((id, i) => id === after[i]!.id);
      expect(old || fresh).toBe(true);
    }
    unsubscribe();
  });
});
