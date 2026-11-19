import { afterEach, describe, expect, it, vi } from "vitest";

import { fetchLatestPosts } from "./blog-posts";

afterEach(() => vi.restoreAllMocks());

describe("fetchLatestPosts", () => {
  it("reads the posts from the landing's own route", async () => {
    const posts = [{ id: "p1", title: "Post", url: "/blog/p1", publishedDate: "2099-06-01", image: null, category: { slug: "news", title: "News", url: "/blog/news" } }];
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(Response.json(posts));
    await expect(fetchLatestPosts()).resolves.toEqual(posts);
    expect(fetchMock).toHaveBeenCalledWith("/create/latest-posts", expect.objectContaining({ headers: { accept: "application/json" } }));
  });

  it("is null on an error status, and drops the body it does not read", async () => {
    const failed = Response.json({ error: "down" }, { status: 503 });
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(failed);
    await expect(fetchLatestPosts()).resolves.toBeNull();
    expect(failed.bodyUsed).toBe(true);
  });

  it("is null when the answer is not a list", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(Response.json({ posts: [] }));
    await expect(fetchLatestPosts()).resolves.toBeNull();
  });
});
