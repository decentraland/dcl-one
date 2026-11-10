// @vitest-environment jsdom
import "../locale/test-setup";
import type { ReactNode } from "react";
import { renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";

import { createFixtureBlogSource } from "@/data/lib/blog/fixture-source";
import type { BlogSource } from "@/data/lib/blog/source";
import type { BlogPost } from "@/data/lib/blog/types";

let source: BlogSource;
vi.mock("@/data/lib/blog/current-source", () => ({
  currentBlogSource: async () => source,
}));

import { RELATED_FETCH_LIMIT, useRelatedPosts } from "./related";

function setup(override?: Partial<BlogSource>) {
  const base = createFixtureBlogSource();
  const listPosts = vi.fn(base.listPosts);
  source = { ...base, listPosts, ...override };
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return { base, listPosts, wrapper };
}

async function announcements(base: BlogSource): Promise<BlogPost[]> {
  return (
    await base.listPosts(
      { scope: "category", slug: "announcements" },
      { skip: 0, limit: 50 },
      new AbortController().signal,
    )
  ).items;
}

describe("useRelatedPosts", () => {
  it("asks for the category's first 4 and shows at most 3, without the current post", async () => {
    const { base, listPosts, wrapper } = setup();
    const posts = await announcements(base);
    const current = posts[1]!;
    const { result } = renderHook(() => useRelatedPosts(current), { wrapper });
    await waitFor(() => expect(result.current).toHaveLength(3));
    expect(listPosts).toHaveBeenCalledWith(
      { scope: "category", slug: "announcements" },
      { skip: 0, limit: RELATED_FETCH_LIMIT },
      expect.any(AbortSignal),
    );
    expect(result.current.map((post) => post.id)).toEqual(
      posts
        .slice(0, 4)
        .filter((post) => post.id !== current.id)
        .map((post) => post.id),
    );
  });

  it("shows the first 3 when the current post is not among the first 4", async () => {
    const { base, wrapper } = setup();
    const posts = await announcements(base);
    const current = posts[posts.length - 1]!;
    const { result } = renderHook(() => useRelatedPosts(current), { wrapper });
    await waitFor(() => expect(result.current).toHaveLength(3));
    expect(result.current.map((post) => post.id)).toEqual(
      posts.slice(0, 3).map((post) => post.id),
    );
  });

  it("shows nothing while loading and on an error", async () => {
    const { base, wrapper } = setup({
      listPosts: vi.fn(async () => Promise.reject(new Error("down"))),
    });
    const current = (await announcements(base))[0]!;
    const { result } = renderHook(() => useRelatedPosts(current), { wrapper });
    expect(result.current).toEqual([]);
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(result.current).toEqual([]);
  });
});
