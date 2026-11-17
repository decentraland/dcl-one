import { afterEach, describe, expect, it, vi } from "vitest";

import { blogCms, BlogUnavailableError, cmsUrl, listPageWindow, searchPageWindow } from "./cms";
import { CMS_BASE, fakeCmsFetch, installFakeCms, requestedUrls } from "./cms.test-utils";

afterEach(() => {
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("request building", () => {
  it("joins the base and path and sets only the params that have a value", () => {
    expect(cmsUrl(CMS_BASE, "/blog/posts", { locale: "en-US", category: undefined, q: "", limit: 7, skip: 0 })).toBe(
      `${CMS_BASE}/blog/posts?locale=en-US&limit=7&skip=0`,
    );
    expect(cmsUrl(CMS_BASE, "/blog/posts", { q: "a&b c" })).toBe(`${CMS_BASE}/blog/posts?q=a%26b+c`);
  });

  it("pages lists 7 then 6, and search 10 at a time", () => {
    expect([1, 2, 3].map(listPageWindow)).toEqual([
      { skip: 0, limit: 7 },
      { skip: 7, limit: 6 },
      { skip: 13, limit: 6 },
    ]);
    expect(searchPageWindow(2)).toEqual({ skip: 10, limit: 10 });
  });

  it("asks for a category's pages, the categories and authors once, and each picture by id", async () => {
    const fetchMock = installFakeCms();
    const cms = blogCms();
    const { posts, total } = await cms.list({ category: "platform" }, 2);
    expect(total).toBe(2);
    expect(posts.map((p) => p.slug)).toEqual(["first-post", "second-post"]);
    const urls = requestedUrls(fetchMock);
    expect(urls).toContain(`${CMS_BASE}/blog/posts?locale=en-US&category=platform&limit=7&skip=0`);
    expect(urls).toContain(`${CMS_BASE}/blog/posts?locale=en-US&category=platform&limit=6&skip=7`);
    expect(urls.filter((u) => u.includes("/blog/categories"))).toEqual([`${CMS_BASE}/blog/categories?locale=en-US&limit=100`]);
    expect(urls.filter((u) => u.includes("/blog/authors"))).toHaveLength(1);
    expect(urls).toContain(`${CMS_BASE}/assets/hero-1?locale=en-US`);

    // A second read of the same pages is served from memory.
    fetchMock.mockClear();
    await cms.list({ category: "platform" }, 2);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("searches with q, the locale and the search page size", async () => {
    const fetchMock = installFakeCms();
    const { posts } = await blogCms().search("news", 1);
    expect(posts).toHaveLength(9);
    expect(requestedUrls(fetchMock)).toContain(`${CMS_BASE}/blog/posts?q=news&locale=en-US&limit=10&skip=0`);
  });

  it("reads a deep search page in one request, capped at the CMS page limit", async () => {
    const fetchMock = installFakeCms();
    await blogCms().search("news", 3);
    expect(requestedUrls(fetchMock).filter((u) => u.includes("q=news"))).toEqual([
      `${CMS_BASE}/blog/posts?q=news&locale=en-US&limit=30&skip=0`,
    ]);

    fetchMock.mockClear();
    await blogCms().search("news", 999);
    expect(requestedUrls(fetchMock).filter((u) => u.includes("q=news"))).toEqual([
      `${CMS_BASE}/blog/posts?q=news&locale=en-US&limit=100&skip=0`,
    ]);
  });
});

describe("response mapping", () => {
  it("maps a post by slug with its author, picture and the pictures its body embeds", async () => {
    installFakeCms();
    const found = await blogCms().post("first-post");
    expect(found?.post).toMatchObject({
      slug: "first-post",
      category: { slug: "platform", title: "Platform" },
      author: { slug: "jane-doe", name: "Jane Doe", image: "https://images.example/jane-face.png" },
      image: { url: "https://images.example/hero-1.png", width: 1200, height: 600, alt: "A plaza at night" },
      publishedAt: "2026-07-02",
    });
    expect(Object.fromEntries(found!.assets)).toEqual({
      "body-image": { url: "https://images.example/body-image.png", width: 800, height: 400, alt: "The new map" },
    });
  });

  it("answers null for a slug the CMS does not have", async () => {
    installFakeCms();
    expect(await blogCms().post("nope")).toBeNull();
  });

  it("keeps list cards without bodies", async () => {
    installFakeCms();
    const { posts } = await blogCms().list({}, 1);
    expect(posts).toHaveLength(7);
    expect(posts.every((p) => p.body === null)).toBe(true);
  });
});

describe("failures", () => {
  it("is unavailable when BLOG_CMS_URL is unset", () => {
    vi.stubEnv("BLOG_CMS_URL", "");
    expect(() => blogCms()).toThrow(BlogUnavailableError);
  });

  it.each(["status", "json", "network"] as const)("is unavailable on a %s failure", async (fail) => {
    installFakeCms(fail);
    await expect(blogCms().list({}, 1)).rejects.toBeInstanceOf(BlogUnavailableError);
    await expect(blogCms().post("first-post")).rejects.toBeInstanceOf(BlogUnavailableError);
  });

  it("does not keep a failed read: the next request asks again", async () => {
    const fetchMock = installFakeCms("status");
    await expect(blogCms().list({}, 1)).rejects.toThrow();
    const calls = fetchMock.mock.calls.length;
    await expect(blogCms().list({}, 1)).rejects.toThrow();
    expect(fetchMock.mock.calls.length).toBeGreaterThan(calls);
  });
});

describe("response bodies", () => {
  const tracked = (status: number) => {
    const response = new Response(new ReadableStream({ start: (c) => c.enqueue(new TextEncoder().encode("error page")) }), { status });
    return response;
  };

  it.each([404, 500, 502])("are released when a list answers %i", async (status) => {
    installFakeCms();
    const response = tracked(status);
    vi.stubGlobal("fetch", vi.fn(async () => response));
    await expect(blogCms().list({}, 1)).rejects.toBeInstanceOf(BlogUnavailableError);
    expect(response.bodyUsed).toBe(true);
  });

  it("are released when a picture is missing (404), and the page still loads", async () => {
    const fake = installFakeCms();
    const responses: Response[] = [];
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: string, init?: RequestInit) => {
        if (!input.includes("/assets/")) return fake(input, init);
        const response = tracked(404);
        responses.push(response);
        return response;
      }),
    );
    const { posts } = await blogCms().list({}, 1);
    expect(posts[0]?.image).toBeNull();
    expect(responses.length).toBeGreaterThan(0);
    expect(responses.every((r) => r.bodyUsed)).toBe(true);
  });
});

describe("bounded caches", () => {
  const postReads = (fetchMock: ReturnType<typeof installFakeCms>, slug: string) =>
    requestedUrls(fetchMock).filter((u) => u.includes(`slug=${slug}`)).length;

  it("keeps an unknown slug's miss for a minute, never a found post", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    const fetchMock = installFakeCms();
    const cms = blogCms();
    expect(await cms.post("nope")).toBeNull();
    expect(await cms.post("nope")).toBeNull();
    expect(postReads(fetchMock, "nope")).toBe(1);
    vi.setSystemTime(Date.now() + 61_000);
    await cms.post("nope");
    expect(postReads(fetchMock, "nope")).toBe(2);

    await cms.post("first-post");
    await cms.post("first-post");
    expect(postReads(fetchMock, "first-post")).toBe(2);
  });

  it("does not keep a failed post read as a miss", async () => {
    const fetchMock = installFakeCms("status");
    await expect(blogCms().post("nope")).rejects.toBeInstanceOf(BlogUnavailableError);
    await expect(blogCms().post("nope")).rejects.toBeInstanceOf(BlogUnavailableError);
    expect(postReads(fetchMock, "nope")).toBe(2);
  });

  it("serves a repeated search from memory, whatever its case and spacing, and evicts the oldest past 200", async () => {
    const fetchMock = installFakeCms();
    const cms = blogCms();
    const searches = () => requestedUrls(fetchMock).filter((u) => u.includes("q=")).length;
    await cms.search("News", 1);
    await cms.search("  news ", 1);
    expect(searches()).toBe(1);
    expect(requestedUrls(fetchMock)).toContain(`${CMS_BASE}/blog/posts?q=news&locale=en-US&limit=10&skip=0`);
    for (let i = 0; i < 200; i++) await cms.search(`query ${i}`, 1);
    expect(searches()).toBe(201);
    await cms.search("news", 1);
    expect(searches()).toBe(202);
  });

  it("does not keep a failed search", async () => {
    const fetchMock = installFakeCms("status");
    await expect(blogCms().search("news", 1)).rejects.toBeInstanceOf(BlogUnavailableError);
    await expect(blogCms().search("news", 1)).rejects.toBeInstanceOf(BlogUnavailableError);
    expect(requestedUrls(fetchMock).filter((u) => u.includes("q=")).length).toBe(2);
  });
});

describe("list pages past their ttl", () => {
  it("are served stale at once while one reload runs, so a slow CMS never empties /create's block", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    let slow = false;
    const fetchMock = installFakeCms(undefined, { hang: () => slow });
    const cms = blogCms();
    const first = await cms.list({}, 1);
    slow = true;
    vi.setSystemTime(Date.now() + 6 * 60_000);
    fetchMock.mockClear();
    expect(await cms.list({}, 1)).toEqual(first);
    expect(await cms.list({}, 1)).toEqual(first);
    // One reload behind the stale answers, not one per caller.
    expect(requestedUrls(fetchMock).filter((u) => u.includes("/blog/posts"))).toHaveLength(1);
  });

  it("are not served once they are older than the stale window", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    installFakeCms();
    const cms = blogCms();
    await cms.list({}, 1);
    vi.setSystemTime(Date.now() + 2 * 60 * 60_000);
    vi.stubGlobal("fetch", fakeCmsFetch("status"));
    await expect(cms.list({}, 1)).rejects.toBeInstanceOf(BlogUnavailableError);
  });
});
