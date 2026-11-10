import { afterEach, describe, expect, it, vi } from "vitest";

import { blogCms, BlogUnavailableError, cmsUrl, listPageWindow, searchPageWindow } from "./cms";
import { CMS_BASE, installFakeCms, requestedUrls } from "./cms.test-utils";

afterEach(() => {
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
});

const signal = () => new AbortController().signal;

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
    const { posts } = await blogCms().search("news", 1, signal());
    expect(posts).toHaveLength(9);
    expect(requestedUrls(fetchMock)).toContain(`${CMS_BASE}/blog/posts?q=news&locale=en-US&limit=10&skip=0`);
  });
});

describe("response mapping", () => {
  it("maps a post by slug with its author, picture and the pictures its body embeds", async () => {
    installFakeCms();
    const found = await blogCms().post("first-post", signal());
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
    expect(await blogCms().post("nope", signal())).toBeNull();
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
    await expect(blogCms().post("first-post", signal())).rejects.toBeInstanceOf(BlogUnavailableError);
  });

  it("does not keep a failed read: the next request asks again", async () => {
    const fetchMock = installFakeCms("status");
    await expect(blogCms().list({}, 1)).rejects.toThrow();
    const calls = fetchMock.mock.calls.length;
    await expect(blogCms().list({}, 1)).rejects.toThrow();
    expect(fetchMock.mock.calls.length).toBeGreaterThan(calls);
  });
});
