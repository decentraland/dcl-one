import { afterEach, describe, expect, it, vi } from "vitest";

import { installFakeCms } from "./cms.test-utils";
import { BLOG_LOADER_TIMEOUT_MS, loadBlogAuthor, loadBlogCategory, loadBlogHome, loadBlogPost, loadBlogSearch } from "./pages";
import type { BlogListData, BlogPostData } from "./types";

afterEach(() => {
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

const req = (path: string) => new Request(`https://sites.test${path}`);

async function list(load: Promise<unknown>) {
  const outcome = (await load) as { status: number; payload: BlogListData };
  expect(outcome.status).toBe(200);
  return outcome.payload;
}

describe("list pages", () => {
  it("home: the first 7 posts newest first, the menu categories and a next page", async () => {
    installFakeCms();
    const data = await list(loadBlogHome(req("/blog")));
    expect(data.posts.map((p) => p.slug)).toEqual(["first-post", "second-post", "news-1", "news-2", "news-3", "news-4", "news-5"]);
    expect(data).toMatchObject({ kind: "home", viewKey: "home", total: 11, activeCategory: "", nextHref: "/blog?page=2" });
    expect(data.categories).toEqual([
      { slug: "platform", title: "Platform" },
      { slug: "announcements", title: "Announcements" },
    ]);
  });

  it("load more: ?page=2 adds the next 6 and ends the list", async () => {
    installFakeCms();
    const data = await list(loadBlogHome(req("/blog?page=2")));
    expect(data.posts).toHaveLength(11);
    expect(data.nextHref).toBeNull();
  });

  it("category: its posts, title and description; a hidden category still has its page", async () => {
    installFakeCms();
    const data = await list(loadBlogCategory(req("/blog/platform"), "platform"));
    expect(data).toMatchObject({ kind: "category", viewKey: "category:platform", title: "Platform", subtitle: "About Platform", total: 2, nextHref: null });
    const hidden = await list(loadBlogCategory(req("/blog/interview"), "interview"));
    expect(hidden).toMatchObject({ total: 0, activeCategory: "interview", posts: [] });
  });

  it("author: the author's posts by slug", async () => {
    installFakeCms();
    const data = await list(loadBlogAuthor(req("/blog/author/jane-doe"), "jane-doe"));
    expect(data).toMatchObject({ kind: "author", viewKey: "author:jane-doe", title: "Jane Doe", activeCategory: null });
    expect(data.posts.map((p) => p.slug)).toEqual(["first-post"]);
  });

  it.each([
    ["category", () => loadBlogCategory(req("/blog/nope"), "nope")],
    ["author", () => loadBlogAuthor(req("/blog/author/nope"), "nope")],
  ])("an unknown %s is a 404", async (kind, load) => {
    installFakeCms();
    expect(await load()).toEqual({ status: 404, payload: { state: "not-found", kind } });
  });
});

describe("search", () => {
  it("sends q to the CMS and pages its results with a next link that keeps q", async () => {
    installFakeCms();
    const data = await list(loadBlogSearch(req("/blog/search?q=%20News%20")));
    expect(data).toMatchObject({ kind: "search", viewKey: null, query: "News", total: 9, nextHref: null });
    expect(data.posts).toHaveLength(9);
  });

  it("redirects an empty or blank q to the home without asking the CMS", async () => {
    const fetchMock = installFakeCms();
    expect(await loadBlogSearch(req("/blog/search?q=%20%20"))).toEqual({ status: 302, location: "/blog" });
    expect(await loadBlogSearch(req("/blog/search"))).toEqual({ status: 302, location: "/blog" });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("does not search under 3 characters and says why", async () => {
    const fetchMock = installFakeCms();
    const data = await list(loadBlogSearch(req("/blog/search?q=ab")));
    expect(data.posts).toEqual([]);
    expect(data.emptyText).toMatch(/at least 3 characters/);
    expect(fetchMock.mock.calls.some(([u]) => String(u).includes("/blog/posts"))).toBe(false);
  });

  it("caps a long q", async () => {
    installFakeCms();
    const data = await list(loadBlogSearch(req(`/blog/search?q=${"n".repeat(300)}`)));
    expect(data.query).toHaveLength(200);
  });
});

describe("post", () => {
  it("answers the post with its body pictures, related posts and share URL", async () => {
    installFakeCms();
    vi.stubEnv("PUBLIC_CATALYST_URL", "https://public.example");
    const outcome = (await loadBlogPost(req("/blog/platform/first-post"), "platform", "first-post")) as { status: number; payload: BlogPostData };
    expect(outcome.status).toBe(200);
    const data = outcome.payload;
    expect(data.post.slug).toBe("first-post");
    expect(Object.keys(data.post.assets)).toEqual(["body-image"]);
    expect(data.shareUrl).toBe("https://public.example/blog/platform/first-post");
    expect(data.related.map((r) => r.slug)).toEqual(["second-post", "news-1", "news-2"]);
  });

  it("is a 404 for an unknown post", async () => {
    installFakeCms();
    expect(await loadBlogPost(req("/blog/platform/nope"), "platform", "nope")).toEqual({ status: 404, payload: { state: "not-found", kind: "post" } });
  });

  it("moves a post reached under another category to its own URL", async () => {
    installFakeCms();
    expect(await loadBlogPost(req("/blog/announcements/first-post"), "announcements", "first-post")).toEqual({
      status: 301,
      location: "/blog/platform/first-post",
    });
  });
});

describe("unavailable", () => {
  const unavailable = { status: 503, payload: { state: "unavailable" } };

  it("is a 503 when BLOG_CMS_URL is unset, without asking anything", async () => {
    const fetchMock = installFakeCms();
    vi.stubEnv("BLOG_CMS_URL", "");
    expect(await loadBlogHome(req("/blog"))).toEqual(unavailable);
    expect(await loadBlogPost(req("/blog/platform/first-post"), "platform", "first-post")).toEqual(unavailable);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it.each(["status", "json", "network"] as const)("is a 503 when the CMS fails (%s)", async (fail) => {
    installFakeCms(fail);
    expect(await loadBlogHome(req("/blog"))).toEqual(unavailable);
    expect(await loadBlogCategory(req("/blog/platform"), "platform")).toEqual(unavailable);
    expect(await loadBlogSearch(req("/blog/search?q=news"))).toEqual(unavailable);
    expect(await loadBlogPost(req("/blog/platform/first-post"), "platform", "first-post")).toEqual(unavailable);
  });

  it("is a 503 when the CMS does not answer before the deadline", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    installFakeCms("hang");
    const pending = loadBlogSearch(req("/blog/search?q=news"));
    await vi.advanceTimersByTimeAsync(BLOG_LOADER_TIMEOUT_MS);
    expect(await pending).toEqual(unavailable);
  });

  it("stops when the visitor leaves instead of answering", async () => {
    installFakeCms("hang");
    const controller = new AbortController();
    const pending = loadBlogSearch(new Request("https://sites.test/blog/search?q=news", { signal: controller.signal }));
    controller.abort();
    await expect(pending).rejects.toBeDefined();
  });
});
