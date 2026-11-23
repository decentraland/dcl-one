import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { blogCms, BlogUnavailableError, cmsUrl, listPageWindow, searchPageWindow } from "./cms";
import { CMS_BASE, fakeCmsFetch, installFakeCms, requestedUrls } from "./cms.test-utils";

const UNAVAILABLE = "The blog is temporarily unavailable.";

afterEach(() => {
  vi.resetAllMocks();
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("request building", () => {
  describe("when building a CMS URL", () => {
    describe("and some params have no value", () => {
      it("should join the base and path and set only the params that have a value", () => {
        expect(cmsUrl(CMS_BASE, "/blog/posts", { locale: "en-US", category: undefined, q: "", limit: 7, skip: 0 })).toBe(
          `${CMS_BASE}/blog/posts?locale=en-US&limit=7&skip=0`,
        );
      });
    });

    describe("and a param has reserved characters", () => {
      it("should encode it", () => {
        expect(cmsUrl(CMS_BASE, "/blog/posts", { q: "a&b c" })).toBe(`${CMS_BASE}/blog/posts?q=a%26b+c`);
      });
    });
  });

  describe("when paging a list", () => {
    it("should page 7 then 6", () => {
      expect([1, 2, 3].map(listPageWindow)).toEqual([
        { skip: 0, limit: 7 },
        { skip: 7, limit: 6 },
        { skip: 13, limit: 6 },
      ]);
    });
  });

  describe("when paging a search", () => {
    it("should page 10 at a time", () => {
      expect(searchPageWindow(2)).toEqual({ skip: 10, limit: 10 });
    });
  });

  describe("when listing a category's pages", () => {
    let fetchMock: ReturnType<typeof installFakeCms>;
    let cms: ReturnType<typeof blogCms>;
    let result: Awaited<ReturnType<ReturnType<typeof blogCms>["list"]>>;
    let urls: string[];

    beforeEach(async () => {
      fetchMock = installFakeCms();
      cms = blogCms();
      result = await cms.list({ category: "platform" }, 2);
      urls = requestedUrls(fetchMock);
    });

    it("should answer the category's total", () => {
      expect(result.total).toBe(2);
    });

    it("should answer the category's posts", () => {
      expect(result.posts.map((p) => p.slug)).toEqual(["first-post", "second-post"]);
    });

    it("should ask for each of the category's pages", () => {
      expect(urls).toContain(`${CMS_BASE}/blog/posts?locale=en-US&category=platform&limit=7&skip=0`);
      expect(urls).toContain(`${CMS_BASE}/blog/posts?locale=en-US&category=platform&limit=6&skip=7`);
    });

    it("should ask for the categories once", () => {
      expect(urls.filter((u) => u.includes("/blog/categories"))).toEqual([`${CMS_BASE}/blog/categories?locale=en-US&limit=100`]);
    });

    it("should ask for the authors once", () => {
      expect(urls.filter((u) => u.includes("/blog/authors"))).toHaveLength(1);
    });

    it("should ask for each picture by id", () => {
      expect(urls).toContain(`${CMS_BASE}/assets/hero-1?locale=en-US`);
    });

    describe("and the same pages are read again", () => {
      beforeEach(async () => {
        fetchMock.mockClear();
        await cms.list({ category: "platform" }, 2);
      });

      it("should serve them from memory", () => {
        expect(fetchMock).not.toHaveBeenCalled();
      });
    });
  });

  describe("when searching", () => {
    let fetchMock: ReturnType<typeof installFakeCms>;

    beforeEach(() => {
      fetchMock = installFakeCms();
    });

    describe("and reading the first page", () => {
      let posts: Awaited<ReturnType<ReturnType<typeof blogCms>["search"]>>["posts"];

      beforeEach(async () => {
        ({ posts } = await blogCms().search("news", 1));
      });

      it("should answer the matches", () => {
        expect(posts).toHaveLength(9);
      });

      it("should search with q, the locale and the search page size", () => {
        expect(requestedUrls(fetchMock)).toContain(`${CMS_BASE}/blog/posts?q=news&locale=en-US&limit=10&skip=0`);
      });
    });

    describe("and reading a deep page", () => {
      it("should read it in one request", async () => {
        await blogCms().search("news", 3);
        expect(requestedUrls(fetchMock).filter((u) => u.includes("q=news"))).toEqual([
          `${CMS_BASE}/blog/posts?q=news&locale=en-US&limit=30&skip=0`,
        ]);
      });
    });

    describe("and reading a page past the CMS page limit", () => {
      it("should cap the request at the CMS page limit", async () => {
        await blogCms().search("news", 999);
        expect(requestedUrls(fetchMock).filter((u) => u.includes("q=news"))).toEqual([
          `${CMS_BASE}/blog/posts?q=news&locale=en-US&limit=100&skip=0`,
        ]);
      });
    });
  });
});

describe("response mapping", () => {
  beforeEach(() => {
    installFakeCms();
  });

  describe("when reading a post by slug", () => {
    let found: Awaited<ReturnType<ReturnType<typeof blogCms>["post"]>>;

    beforeEach(async () => {
      found = await blogCms().post("first-post");
    });

    it("should map the post with its author and picture", () => {
      expect(found?.post).toMatchObject({
        slug: "first-post",
        category: { slug: "platform", title: "Platform" },
        author: { slug: "jane-doe", name: "Jane Doe", image: "https://images.example/jane-face.png" },
        image: { url: "https://images.example/hero-1.png", width: 1200, height: 600, alt: "A plaza at night" },
        publishedAt: "2026-07-02",
      });
    });

    it("should map the pictures its body embeds", () => {
      expect(Object.fromEntries(found!.assets)).toEqual({
        "body-image": { url: "https://images.example/body-image.png", width: 800, height: 400, alt: "The new map" },
      });
    });
  });

  describe("when the CMS does not have the slug", () => {
    it("should answer null", async () => {
      expect(await blogCms().post("nope")).toBeNull();
    });
  });

  describe("when reading a list", () => {
    let posts: Awaited<ReturnType<ReturnType<typeof blogCms>["list"]>>["posts"];

    beforeEach(async () => {
      ({ posts } = await blogCms().list({}, 1));
    });

    it("should answer the page's cards", () => {
      expect(posts).toHaveLength(7);
    });

    it("should keep the cards without bodies", () => {
      expect(posts.every((p) => p.body === null)).toBe(true);
    });
  });
});

describe("failures", () => {
  describe("when BLOG_CMS_URL is unset", () => {
    beforeEach(() => {
      vi.stubEnv("BLOG_CMS_URL", "");
    });

    it("should be unavailable", () => {
      expect(() => blogCms()).toThrow(BlogUnavailableError);
    });

    it("should say the blog is unavailable", () => {
      expect(() => blogCms()).toThrow(UNAVAILABLE);
    });
  });

  describe.each(["status", "json", "network"] as const)("when the CMS has a %s failure", (fail) => {
    beforeEach(() => {
      installFakeCms(fail);
    });

    it("should make a list unavailable", async () => {
      await expect(blogCms().list({}, 1)).rejects.toBeInstanceOf(BlogUnavailableError);
    });

    it("should make a post unavailable", async () => {
      await expect(blogCms().post("first-post")).rejects.toBeInstanceOf(BlogUnavailableError);
    });

    it("should say the blog is unavailable", async () => {
      await expect(blogCms().list({}, 1)).rejects.toThrow(UNAVAILABLE);
    });
  });

  describe("when a read failed", () => {
    let fetchMock: ReturnType<typeof installFakeCms>;

    beforeEach(() => {
      fetchMock = installFakeCms("status");
    });

    it("should not keep it, so the next request asks again", async () => {
      await expect(blogCms().list({}, 1)).rejects.toThrow(UNAVAILABLE);
      const calls = fetchMock.mock.calls.length;
      await expect(blogCms().list({}, 1)).rejects.toThrow(UNAVAILABLE);
      expect(fetchMock.mock.calls.length).toBeGreaterThan(calls);
    });
  });
});

describe("response bodies", () => {
  const tracked = (status: number) => {
    const response = new Response(new ReadableStream({ start: (c) => c.enqueue(new TextEncoder().encode("error page")) }), { status });
    return response;
  };

  describe.each([404, 500, 502])("when a list answers %i", (status) => {
    let response: Response;
    let result: Promise<unknown>;

    beforeEach(() => {
      installFakeCms();
      response = tracked(status);
      vi.stubGlobal("fetch", vi.fn(async () => response));
      result = blogCms().list({}, 1);
    });

    it("should be unavailable", async () => {
      await expect(result).rejects.toBeInstanceOf(BlogUnavailableError);
    });

    it("should release the body", async () => {
      await result.catch(() => undefined);
      expect(response.bodyUsed).toBe(true);
    });
  });

  describe("when a picture is missing (404)", () => {
    let responses: Response[];
    let posts: Awaited<ReturnType<ReturnType<typeof blogCms>["list"]>>["posts"];

    beforeEach(async () => {
      const fake = installFakeCms();
      responses = [];
      vi.stubGlobal(
        "fetch",
        vi.fn(async (input: string, init?: RequestInit) => {
          if (!input.includes("/assets/")) return fake(input, init);
          const response = tracked(404);
          responses.push(response);
          return response;
        }),
      );
      ({ posts } = await blogCms().list({}, 1));
    });

    it("should still load the page without the picture", () => {
      expect(posts[0]?.image).toBeNull();
    });

    it("should release every picture's body", () => {
      expect(responses.length).toBeGreaterThan(0);
      expect(responses.every((r) => r.bodyUsed)).toBe(true);
    });
  });
});

describe("bounded caches", () => {
  const postReads = (fetchMock: ReturnType<typeof installFakeCms>, slug: string) =>
    requestedUrls(fetchMock).filter((u) => u.includes(`slug=${slug}`)).length;

  describe("when an unknown slug is read", () => {
    let fetchMock: ReturnType<typeof installFakeCms>;
    let cms: ReturnType<typeof blogCms>;

    beforeEach(() => {
      vi.useFakeTimers({ toFake: ["Date"] });
      fetchMock = installFakeCms();
      cms = blogCms();
    });

    it("should keep the miss for a minute", async () => {
      expect(await cms.post("nope")).toBeNull();
      expect(await cms.post("nope")).toBeNull();
      expect(postReads(fetchMock, "nope")).toBe(1);
      vi.setSystemTime(Date.now() + 61_000);
      await cms.post("nope");
      expect(postReads(fetchMock, "nope")).toBe(2);
    });

    it("should never keep a found post", async () => {
      await cms.post("first-post");
      await cms.post("first-post");
      expect(postReads(fetchMock, "first-post")).toBe(2);
    });
  });

  describe("when a post read fails", () => {
    let fetchMock: ReturnType<typeof installFakeCms>;

    beforeEach(() => {
      fetchMock = installFakeCms("status");
    });

    it("should not keep it as a miss", async () => {
      await expect(blogCms().post("nope")).rejects.toBeInstanceOf(BlogUnavailableError);
      await expect(blogCms().post("nope")).rejects.toBeInstanceOf(BlogUnavailableError);
      expect(postReads(fetchMock, "nope")).toBe(2);
    });
  });

  describe("when a search is repeated", () => {
    let fetchMock: ReturnType<typeof installFakeCms>;
    let cms: ReturnType<typeof blogCms>;
    const searches = () => requestedUrls(fetchMock).filter((u) => u.includes("q=")).length;

    beforeEach(async () => {
      fetchMock = installFakeCms();
      cms = blogCms();
      await cms.search("News", 1);
      await cms.search("  news ", 1);
    });

    it("should serve it from memory, whatever its case and spacing", () => {
      expect(searches()).toBe(1);
      expect(requestedUrls(fetchMock)).toContain(`${CMS_BASE}/blog/posts?q=news&locale=en-US&limit=10&skip=0`);
    });

    describe("and more than 200 other searches follow", () => {
      it("should evict the oldest", async () => {
        for (let i = 0; i < 200; i++) await cms.search(`query ${i}`, 1);
        expect(searches()).toBe(201);
        await cms.search("news", 1);
        expect(searches()).toBe(202);
      });
    });
  });

  describe("when a search fails", () => {
    let fetchMock: ReturnType<typeof installFakeCms>;

    beforeEach(() => {
      fetchMock = installFakeCms("status");
    });

    it("should not keep it", async () => {
      await expect(blogCms().search("news", 1)).rejects.toBeInstanceOf(BlogUnavailableError);
      await expect(blogCms().search("news", 1)).rejects.toBeInstanceOf(BlogUnavailableError);
      expect(requestedUrls(fetchMock).filter((u) => u.includes("q=")).length).toBe(2);
    });
  });
});

describe("list pages past their ttl", () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["Date"] });
  });

  describe("when the CMS is slow to reload them", () => {
    let fetchMock: ReturnType<typeof installFakeCms>;
    let cms: ReturnType<typeof blogCms>;
    let first: Awaited<ReturnType<ReturnType<typeof blogCms>["list"]>>;

    beforeEach(async () => {
      let slow = false;
      fetchMock = installFakeCms(undefined, { hang: () => slow });
      cms = blogCms();
      first = await cms.list({}, 1);
      slow = true;
      vi.setSystemTime(Date.now() + 6 * 60_000);
      fetchMock.mockClear();
    });

    it("should serve them stale at once while one reload runs, so a slow CMS never empties /create's block", async () => {
      expect(await cms.list({}, 1)).toEqual(first);
      expect(await cms.list({}, 1)).toEqual(first);
      // One reload behind the stale answers, not one per caller.
      expect(requestedUrls(fetchMock).filter((u) => u.includes("/blog/posts"))).toHaveLength(1);
    });
  });

  describe("when they are older than the stale window", () => {
    let cms: ReturnType<typeof blogCms>;

    beforeEach(async () => {
      installFakeCms();
      cms = blogCms();
      await cms.list({}, 1);
      vi.setSystemTime(Date.now() + 2 * 60 * 60_000);
      vi.stubGlobal("fetch", fakeCmsFetch("status"));
    });

    it("should not serve them", async () => {
      await expect(cms.list({}, 1)).rejects.toBeInstanceOf(BlogUnavailableError);
    });
  });
});
