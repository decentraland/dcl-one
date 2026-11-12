import { afterEach, describe, expect, it, vi } from "vitest";

import { category, CMS_CATEGORIES, CMS_POSTS, installFakeCms, post, requestedUrls } from "./cms.test-utils";
import { BLOG_LOADER_TIMEOUT_MS, BLOG_RELATED_TIMEOUT_MS, loadBlogAuthor, loadBlogCategory, loadBlogHome, loadBlogPost, loadBlogSearch } from "./pages";
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
    expect(data.related?.map((r) => r.slug)).toEqual(["second-post", "news-1", "news-2"]);
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

  const lists = (url: URL) => url.pathname.endsWith("/blog/posts") && !url.searchParams.has("slug") && !url.searchParams.has("q");

  it("renders the post without a related section when the related lists fail", async () => {
    installFakeCms(undefined, { fail: (url) => (lists(url) ? new Response("down", { status: 502 }) : undefined) });
    const outcome = (await loadBlogPost(req("/blog/platform/first-post"), "platform", "first-post")) as { status: number; payload: BlogPostData };
    expect(outcome.status).toBe(200);
    expect(outcome.payload.post.slug).toBe("first-post");
    expect(outcome.payload.related).toBeNull();
  });

  it("renders the post without a related section when the related lists outlast their budget", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    installFakeCms(undefined, { hang: lists });
    const pending = loadBlogPost(req("/blog/platform/first-post"), "platform", "first-post");
    await vi.advanceTimersByTimeAsync(BLOG_RELATED_TIMEOUT_MS);
    const outcome = (await pending) as { status: number; payload: BlogPostData };
    expect(outcome.status).toBe(200);
    expect(outcome.payload.related).toBeNull();
  });

  it("still stops when the visitor leaves while the related lists load", async () => {
    installFakeCms(undefined, { hang: lists });
    const controller = new AbortController();
    const pending = loadBlogPost(new Request("https://sites.test/blog/platform/first-post", { signal: controller.signal }), "platform", "first-post");
    setTimeout(() => controller.abort(), 20);
    await expect(pending).rejects.toBeDefined();
  });

  describe("a post whose category the cached list does not have", () => {
    const fresh = post({ id: "p-new", slug: "new-post", title: "New Post", category: "cat-fresh", date: "2026-07-03T00:00:00Z" });
    const categoryReads = (fetchMock: ReturnType<typeof installFakeCms>) => requestedUrls(fetchMock).filter((u) => u.includes("/blog/categories")).length;

    it("reloads the categories once and redirects to the category the CMS now has", async () => {
      let categories = CMS_CATEGORIES;
      const fetchMock = installFakeCms(undefined, { posts: () => [fresh, ...CMS_POSTS], categories: () => categories });
      await loadBlogHome(req("/blog"));
      categories = [...CMS_CATEGORIES, category("cat-fresh", "fresh", "Fresh")];
      expect(await loadBlogPost(req("/blog/platform/new-post"), "platform", "new-post")).toEqual({ status: 301, location: "/blog/fresh/new-post" });
      expect(categoryReads(fetchMock)).toBe(2);
    });

    it("renders at the requested URL, never a redirect to a made-up category, and reloads at most once a minute", async () => {
      const fetchMock = installFakeCms(undefined, { posts: () => [fresh, ...CMS_POSTS] });
      vi.stubEnv("PUBLIC_CATALYST_URL", "https://public.example");
      const outcome = (await loadBlogPost(req("/blog/platform/new-post"), "platform", "new-post")) as { status: number; payload: BlogPostData };
      expect(outcome.status).toBe(200);
      expect(outcome.payload.post.slug).toBe("new-post");
      expect(outcome.payload.shareUrl).toBe("https://public.example/blog/platform/new-post");
      expect(outcome.payload.related?.map((r) => r.slug)).toEqual(["first-post", "second-post", "news-1"]);
      expect(categoryReads(fetchMock)).toBe(2);
      expect((await loadBlogPost(req("/blog/announcements/new-post"), "announcements", "new-post")).status).toBe(200);
      expect(categoryReads(fetchMock)).toBe(2);
    });

    it("renders a post without any category link at the requested URL", async () => {
      const bare = post({ id: "p-bare", slug: "bare-post", title: "Bare", category: "", date: "2026-07-03T00:00:00Z" });
      delete bare.fields!.category;
      installFakeCms(undefined, { posts: () => [bare, ...CMS_POSTS] });
      expect((await loadBlogPost(req("/blog/platform/bare-post"), "platform", "bare-post")).status).toBe(200);
    });
  });
});

describe("old /blog/<post-slug> links", () => {
  it("redirect a post slug to the post's URL", async () => {
    installFakeCms();
    expect(await loadBlogCategory(req("/blog/second-post"), "second-post")).toEqual({ status: 301, location: "/blog/platform/second-post" });
  });

  it("stay a 404 when the slug is no category and no post, or the post's category is unknown", async () => {
    const orphan = post({ id: "p-orphan", slug: "orphan", title: "Orphan", category: "cat-gone", date: "2026-07-03T00:00:00Z" });
    installFakeCms(undefined, { posts: () => [orphan, ...CMS_POSTS] });
    expect(await loadBlogCategory(req("/blog/nope"), "nope")).toEqual({ status: 404, payload: { state: "not-found", kind: "category" } });
    expect(await loadBlogCategory(req("/blog/orphan"), "orphan")).toEqual({ status: 404, payload: { state: "not-found", kind: "category" } });
  });
});

describe("reserved category slugs", () => {
  it("leave a `search` or `author` category out of the tabs and its page, and log it once", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const fetchMock = installFakeCms(undefined, {
      categories: () => [...CMS_CATEGORIES, category("cat-search", "search", "Search"), category("cat-author", "author", "Author")],
    });
    const data = await list(loadBlogHome(req("/blog")));
    expect(data.categories.map((c) => c.slug)).toEqual(["platform", "announcements"]);
    expect((await loadBlogCategory(req("/blog/search"), "search")).status).toBe(404);
    vi.setSystemTime(Date.now() + 10 * 60_000);
    await loadBlogHome(req("/blog"));
    expect(requestedUrls(fetchMock).filter((u) => u.includes("/blog/categories")).length).toBeGreaterThan(1);
    expect(warn).toHaveBeenCalledTimes(2);
    expect(warn.mock.calls.map(([m]) => String(m))).toEqual([expect.stringContaining('"search"'), expect.stringContaining('"author"')]);
    warn.mockRestore();
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
