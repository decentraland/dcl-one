import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from "vitest";

import { category, CMS_CATEGORIES, CMS_POSTS, installFakeCms, post, requestedUrls } from "./cms.test-utils";
import { BLOG_LOADER_TIMEOUT_MS, BLOG_RELATED_TIMEOUT_MS, loadBlogAuthor, loadBlogCategory, loadBlogHome, loadBlogPost, loadBlogSearch } from "./pages";
import type { BlogListData, BlogPostData } from "./types";

afterEach(() => {
  vi.restoreAllMocks();
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

type PostOutcome = { status: number; payload: BlogPostData };

describe("list pages", () => {
  beforeEach(() => {
    installFakeCms();
  });

  describe("when loading the home", () => {
    let data: BlogListData;

    beforeEach(async () => {
      data = await list(loadBlogHome(req("/blog")));
    });

    it("should list the first 7 posts newest first", () => {
      expect(data.posts.map((p) => p.slug)).toEqual(["first-post", "second-post", "news-1", "news-2", "news-3", "news-4", "news-5"]);
    });

    it("should link the next page", () => {
      expect(data).toMatchObject({ kind: "home", viewKey: "home", total: 11, activeCategory: "", authorSlug: null, nextHref: "/blog?page=2" });
    });

    it("should list the menu categories", () => {
      expect(data.categories).toEqual([
        { slug: "platform", title: "Platform" },
        { slug: "announcements", title: "Announcements" },
      ]);
    });
  });

  describe("when loading more with ?page=2", () => {
    let data: BlogListData;

    beforeEach(async () => {
      data = await list(loadBlogHome(req("/blog?page=2")));
    });

    it("should add the next 6", () => {
      expect(data.posts).toHaveLength(11);
    });

    it("should end the list", () => {
      expect(data.nextHref).toBeNull();
    });
  });

  describe("when loading a category", () => {
    it("should answer its posts, title and description", async () => {
      const data = await list(loadBlogCategory(req("/blog/platform"), "platform"));
      expect(data).toMatchObject({ kind: "category", viewKey: "category:platform", title: "Platform", subtitle: "About Platform", total: 2, nextHref: null });
    });

    describe("and the category is hidden from the menu", () => {
      it("should still have its page", async () => {
        const hidden = await list(loadBlogCategory(req("/blog/interview"), "interview"));
        expect(hidden).toMatchObject({ total: 0, activeCategory: "interview", posts: [] });
      });
    });
  });

  describe("when loading an author", () => {
    let data: BlogListData;

    beforeEach(async () => {
      data = await list(loadBlogAuthor(req("/blog/author/jane-doe"), "jane-doe"));
    });

    it("should answer the author's page by slug", () => {
      expect(data).toMatchObject({ kind: "author", viewKey: "author:jane-doe", title: "Jane Doe", activeCategory: null, authorSlug: "jane-doe" });
    });

    it("should list the author's posts", () => {
      expect(data.posts.map((p) => p.slug)).toEqual(["first-post"]);
    });
  });

  describe.each([
    ["category", () => loadBlogCategory(req("/blog/nope"), "nope")],
    ["author", () => loadBlogAuthor(req("/blog/author/nope"), "nope")],
  ])("when the %s is unknown", (kind, load) => {
    it("should answer a 404", async () => {
      expect(await load()).toEqual({ status: 404, payload: { state: "not-found", kind } });
    });
  });
});

describe("search", () => {
  let fetchMock: ReturnType<typeof installFakeCms>;

  beforeEach(() => {
    fetchMock = installFakeCms();
  });

  describe("when the query has matches", () => {
    let data: BlogListData;

    beforeEach(async () => {
      data = await list(loadBlogSearch(req("/blog/search?q=%20News%20")));
    });

    it("should send q to the CMS and page its results with a next link that keeps q", () => {
      expect(data).toMatchObject({ kind: "search", viewKey: null, query: "News", total: 9, nextHref: null });
    });

    it("should list the results", () => {
      expect(data.posts).toHaveLength(9);
    });
  });

  describe("when q is blank", () => {
    it("should redirect to the home", async () => {
      expect(await loadBlogSearch(req("/blog/search?q=%20%20"))).toEqual({ status: 302, location: "/blog" });
    });

    it("should not ask the CMS", async () => {
      await loadBlogSearch(req("/blog/search?q=%20%20"));
      expect(fetchMock).not.toHaveBeenCalled();
    });
  });

  describe("when q is missing", () => {
    it("should redirect to the home", async () => {
      expect(await loadBlogSearch(req("/blog/search"))).toEqual({ status: 302, location: "/blog" });
    });
  });

  describe("when q is under 3 characters", () => {
    let data: BlogListData;

    beforeEach(async () => {
      data = await list(loadBlogSearch(req("/blog/search?q=ab")));
    });

    it("should answer no posts", () => {
      expect(data.posts).toEqual([]);
    });

    it("should say why", () => {
      expect(data.emptyText).toMatch(/at least 3 characters/);
    });

    it("should not search the CMS", () => {
      expect(fetchMock.mock.calls.some(([u]) => String(u).includes("/blog/posts"))).toBe(false);
    });
  });

  describe("when q is long", () => {
    it("should cap it", async () => {
      const data = await list(loadBlogSearch(req(`/blog/search?q=${"n".repeat(300)}`)));
      expect(data.query).toHaveLength(200);
    });
  });
});

describe("post", () => {
  const lists = (url: URL) => url.pathname.endsWith("/blog/posts") && !url.searchParams.has("slug") && !url.searchParams.has("q");

  describe("when the post exists", () => {
    let outcome: PostOutcome;

    beforeEach(async () => {
      installFakeCms();
      vi.stubEnv("PUBLIC_CATALYST_URL", "https://public.example");
      outcome = (await loadBlogPost(req("/blog/platform/first-post"), "platform", "first-post")) as PostOutcome;
    });

    it("should answer with a 200 status", () => {
      expect(outcome.status).toBe(200);
    });

    it("should answer the post", () => {
      expect(outcome.payload.post.slug).toBe("first-post");
    });

    it("should answer its body pictures", () => {
      expect(Object.keys(outcome.payload.post.assets)).toEqual(["body-image"]);
    });

    it("should answer its share URL", () => {
      expect(outcome.payload.shareUrl).toBe("https://public.example/blog/platform/first-post");
    });

    it("should answer the CMS's raw publishedDate beside the post's display date", () => {
      expect(outcome.payload.publishedDate).toBe("2026-07-02T10:00-07:00");
      expect(outcome.payload.post.publishedAt).toBe("2026-07-02");
      expect(outcome.payload.post).not.toHaveProperty("publishedDate");
    });

    it("should answer its related posts", () => {
      expect(outcome.payload.related?.map((r) => r.slug)).toEqual(["second-post", "news-1", "news-2"]);
    });
  });

  describe("when the post is unknown", () => {
    beforeEach(() => {
      installFakeCms();
    });

    it("should answer a 404", async () => {
      expect(await loadBlogPost(req("/blog/platform/nope"), "platform", "nope")).toEqual({ status: 404, payload: { state: "not-found", kind: "post" } });
    });
  });

  describe("when the post is reached under another category", () => {
    beforeEach(() => {
      installFakeCms();
    });

    it("should move it to its own URL", async () => {
      expect(await loadBlogPost(req("/blog/announcements/first-post"), "announcements", "first-post")).toEqual({
        status: 301,
        location: "/blog/platform/first-post",
      });
    });
  });

  describe("when the related lists fail", () => {
    let outcome: PostOutcome;

    beforeEach(async () => {
      installFakeCms(undefined, { fail: (url) => (lists(url) ? new Response("down", { status: 502 }) : undefined) });
      outcome = (await loadBlogPost(req("/blog/platform/first-post"), "platform", "first-post")) as PostOutcome;
    });

    it("should still render the post", () => {
      expect(outcome.status).toBe(200);
      expect(outcome.payload.post.slug).toBe("first-post");
    });

    it("should render no related section", () => {
      expect(outcome.payload.related).toBeNull();
    });
  });

  describe("when the related lists outlast their budget", () => {
    let outcome: PostOutcome;

    beforeEach(async () => {
      vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
      installFakeCms(undefined, { hang: lists });
      const pending = loadBlogPost(req("/blog/platform/first-post"), "platform", "first-post");
      await vi.advanceTimersByTimeAsync(BLOG_RELATED_TIMEOUT_MS);
      outcome = (await pending) as PostOutcome;
    });

    it("should still render the post", () => {
      expect(outcome.status).toBe(200);
    });

    it("should render no related section", () => {
      expect(outcome.payload.related).toBeNull();
    });
  });

  describe("when the visitor leaves while the related lists load", () => {
    let controller: AbortController;

    beforeEach(() => {
      installFakeCms(undefined, { hang: lists });
      controller = new AbortController();
    });

    it("should still stop", async () => {
      const pending = loadBlogPost(new Request("https://sites.test/blog/platform/first-post", { signal: controller.signal }), "platform", "first-post");
      setTimeout(() => controller.abort(), 20);
      await expect(pending).rejects.toThrow("This operation was aborted");
    });
  });

  describe("when the cached category list does not have the post's category", () => {
    let fresh: ReturnType<typeof post>;
    const categoryReads = (fetchMock: ReturnType<typeof installFakeCms>) => requestedUrls(fetchMock).filter((u) => u.includes("/blog/categories")).length;

    beforeEach(() => {
      fresh = post({ id: "p-new", slug: "new-post", title: "New Post", category: "cat-fresh", date: "2026-07-03T00:00:00Z" });
    });

    describe("and the CMS now has the category", () => {
      it("should reload the categories once and redirect to the category", async () => {
        let categories = CMS_CATEGORIES;
        const fetchMock = installFakeCms(undefined, { posts: () => [fresh, ...CMS_POSTS], categories: () => categories });
        await loadBlogHome(req("/blog"));
        categories = [...CMS_CATEGORIES, category("cat-fresh", "fresh", "Fresh")];
        expect(await loadBlogPost(req("/blog/platform/new-post"), "platform", "new-post")).toEqual({ status: 301, location: "/blog/fresh/new-post" });
        expect(categoryReads(fetchMock)).toBe(2);
      });
    });

    describe("and the CMS still does not have the category", () => {
      let fetchMock: ReturnType<typeof installFakeCms>;
      let outcome: PostOutcome;

      beforeEach(async () => {
        fetchMock = installFakeCms(undefined, { posts: () => [fresh, ...CMS_POSTS] });
        vi.stubEnv("PUBLIC_CATALYST_URL", "https://public.example");
        outcome = (await loadBlogPost(req("/blog/platform/new-post"), "platform", "new-post")) as PostOutcome;
      });

      it("should render at the requested URL, never a redirect to a made-up category", () => {
        expect(outcome.status).toBe(200);
        expect(outcome.payload.post.slug).toBe("new-post");
        expect(outcome.payload.shareUrl).toBe("https://public.example/blog/platform/new-post");
      });

      it("should answer its related posts", () => {
        expect(outcome.payload.related?.map((r) => r.slug)).toEqual(["first-post", "second-post", "news-1"]);
      });

      it("should reload the categories at most once a minute", async () => {
        expect(categoryReads(fetchMock)).toBe(2);
        expect((await loadBlogPost(req("/blog/announcements/new-post"), "announcements", "new-post")).status).toBe(200);
        expect(categoryReads(fetchMock)).toBe(2);
      });
    });

    describe("and the post has no category link at all", () => {
      beforeEach(() => {
        const bare = post({ id: "p-bare", slug: "bare-post", title: "Bare", category: "", date: "2026-07-03T00:00:00Z" });
        delete bare.fields!.category;
        installFakeCms(undefined, { posts: () => [bare, ...CMS_POSTS] });
      });

      it("should render at the requested URL", async () => {
        expect((await loadBlogPost(req("/blog/platform/bare-post"), "platform", "bare-post")).status).toBe(200);
      });
    });
  });
});

describe("old /blog/<post-slug> links", () => {
  describe("when the slug is a post", () => {
    beforeEach(() => {
      installFakeCms();
    });

    it("should redirect to the post's URL", async () => {
      expect(await loadBlogCategory(req("/blog/second-post"), "second-post")).toEqual({ status: 301, location: "/blog/platform/second-post" });
    });
  });

  describe("when the slug is no category and no post, or the post's category is unknown", () => {
    beforeEach(() => {
      const orphan = post({ id: "p-orphan", slug: "orphan", title: "Orphan", category: "cat-gone", date: "2026-07-03T00:00:00Z" });
      installFakeCms(undefined, { posts: () => [orphan, ...CMS_POSTS] });
    });

    describe("and the slug is no category and no post", () => {
      it("should stay a 404", async () => {
        expect(await loadBlogCategory(req("/blog/nope"), "nope")).toEqual({ status: 404, payload: { state: "not-found", kind: "category" } });
      });
    });

    describe("and the post's category is unknown", () => {
      it("should stay a 404", async () => {
        expect(await loadBlogCategory(req("/blog/orphan"), "orphan")).toEqual({ status: 404, payload: { state: "not-found", kind: "category" } });
      });
    });
  });
});

describe("reserved category slugs", () => {
  describe("when the CMS has a `search` or `author` category", () => {
    let warn: MockInstance<typeof console.warn>;
    let fetchMock: ReturnType<typeof installFakeCms>;
    let data: BlogListData;

    beforeEach(async () => {
      vi.useFakeTimers({ toFake: ["Date"] });
      warn = vi.spyOn(console, "warn").mockImplementation(() => {});
      fetchMock = installFakeCms(undefined, {
        categories: () => [...CMS_CATEGORIES, category("cat-search", "search", "Search"), category("cat-author", "author", "Author")],
      });
      data = await list(loadBlogHome(req("/blog")));
    });

    it("should leave it out of the tabs", () => {
      expect(data.categories.map((c) => c.slug)).toEqual(["platform", "announcements"]);
    });

    it("should leave out its page", async () => {
      expect((await loadBlogCategory(req("/blog/search"), "search")).status).toBe(404);
    });

    it("should log it once across category reloads", async () => {
      await loadBlogCategory(req("/blog/search"), "search");
      vi.setSystemTime(Date.now() + 10 * 60_000);
      await loadBlogHome(req("/blog"));
      expect(requestedUrls(fetchMock).filter((u) => u.includes("/blog/categories")).length).toBeGreaterThan(1);
      expect(warn).toHaveBeenCalledTimes(2);
      expect(warn.mock.calls.map(([m]) => String(m))).toEqual([expect.stringContaining('"search"'), expect.stringContaining('"author"')]);
    });
  });
});

describe("unavailable", () => {
  let unavailable: { status: number; payload: { state: string } };

  beforeEach(() => {
    unavailable = { status: 503, payload: { state: "unavailable" } };
  });

  describe("when BLOG_CMS_URL is unset", () => {
    let fetchMock: ReturnType<typeof installFakeCms>;

    beforeEach(() => {
      fetchMock = installFakeCms();
      vi.stubEnv("BLOG_CMS_URL", "");
    });

    it("should answer the home with a 503", async () => {
      expect(await loadBlogHome(req("/blog"))).toEqual(unavailable);
    });

    it("should answer a post with a 503", async () => {
      expect(await loadBlogPost(req("/blog/platform/first-post"), "platform", "first-post")).toEqual(unavailable);
    });

    it("should not ask anything", async () => {
      await loadBlogHome(req("/blog"));
      await loadBlogPost(req("/blog/platform/first-post"), "platform", "first-post");
      expect(fetchMock).not.toHaveBeenCalled();
    });
  });

  describe.each(["status", "json", "network"] as const)("when the CMS fails (%s)", (fail) => {
    beforeEach(() => {
      installFakeCms(fail);
    });

    it("should answer the home with a 503", async () => {
      expect(await loadBlogHome(req("/blog"))).toEqual(unavailable);
    });

    it("should answer a category with a 503", async () => {
      expect(await loadBlogCategory(req("/blog/platform"), "platform")).toEqual(unavailable);
    });

    it("should answer a search with a 503", async () => {
      expect(await loadBlogSearch(req("/blog/search?q=news"))).toEqual(unavailable);
    });

    it("should answer a post with a 503", async () => {
      expect(await loadBlogPost(req("/blog/platform/first-post"), "platform", "first-post")).toEqual(unavailable);
    });
  });

  describe("when the CMS does not answer before the deadline", () => {
    let pending: ReturnType<typeof loadBlogSearch>;

    beforeEach(async () => {
      vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
      installFakeCms("hang");
      pending = loadBlogSearch(req("/blog/search?q=news"));
      await vi.advanceTimersByTimeAsync(BLOG_LOADER_TIMEOUT_MS);
    });

    it("should answer a 503", async () => {
      expect(await pending).toEqual(unavailable);
    });
  });

  describe("when the visitor leaves", () => {
    let controller: AbortController;

    beforeEach(() => {
      installFakeCms("hang");
      controller = new AbortController();
    });

    it("should stop instead of answering", async () => {
      const pending = loadBlogSearch(new Request("https://sites.test/blog/search?q=news", { signal: controller.signal }));
      controller.abort();
      await expect(pending).rejects.toThrow("This operation was aborted");
    });
  });
});
