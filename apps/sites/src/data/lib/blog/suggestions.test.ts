import { afterEach, describe, expect, it, vi } from "vitest";

import { loader } from "@/routes/routes/api.blog.search";

import { installFakeCms, requestedUrls } from "./cms.test-utils";
import { BLOG_LOADER_TIMEOUT_MS, loadBlogSearch, loadBlogSearchSuggestions } from "./pages";

afterEach(() => {
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

const req = (path: string, init?: RequestInit) => new Request(`https://sites.test${path}`, init);
const searches = (fetchMock: Parameters<typeof requestedUrls>[0]) =>
  requestedUrls(fetchMock).filter((u) => u.includes("/blog/posts") && u.includes("q="));

describe("loadBlogSearchSuggestions", () => {
  it("answers the first 4 matches with their URLs and the full search's total", async () => {
    installFakeCms();
    const outcome = await loadBlogSearchSuggestions(req("/api/blog/search?q=%20News%20"));
    expect(outcome.status).toBe(200);
    if (outcome.status !== 200 || !("payload" in outcome)) throw new Error("expected a payload");
    const data = outcome.payload as { query: string; total: number; posts: { slug: string; href: string; title: string; description: string }[] };
    expect(data.query).toBe("News");
    expect(data.total).toBe(9);
    expect(data.posts).toHaveLength(4);
    for (const post of data.posts) {
      expect(post.href).toMatch(new RegExp(`^/blog/[a-z-]+/${post.slug}$`));
      expect(post.description).toBe(`${post.title} in short`);
      expect(post).not.toHaveProperty("body");
    }
  });

  it("asks nothing for a query under 3 characters or a blank one", async () => {
    const fetchMock = installFakeCms();
    for (const q of ["", "%20%20", "ab", "%20ab%20"]) {
      const outcome = await loadBlogSearchSuggestions(req(`/api/blog/search?q=${q}`));
      expect(outcome).toMatchObject({ status: 200, payload: { posts: [], total: 0 } });
    }
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("shares the results page's search: suggestions then the full search ask the CMS once", async () => {
    const fetchMock = installFakeCms();
    await loadBlogSearchSuggestions(req("/api/blog/search?q=news"));
    await loadBlogSearch(req("/blog/search?q=NEWS"));
    expect(searches(fetchMock)).toHaveLength(1);
  });

  it.each(["status", "json", "network"] as const)("is unavailable when the CMS fails (%s)", async (fail) => {
    installFakeCms(fail);
    expect(await loadBlogSearchSuggestions(req("/api/blog/search?q=news"))).toEqual({ status: 503, payload: { state: "unavailable" } });
  });

  it("is unavailable when the CMS does not answer before the deadline", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    installFakeCms("hang");
    const pending = loadBlogSearchSuggestions(req("/api/blog/search?q=news"));
    await vi.advanceTimersByTimeAsync(BLOG_LOADER_TIMEOUT_MS);
    expect(await pending).toEqual({ status: 503, payload: { state: "unavailable" } });
  });
});

describe("GET /api/blog/search", () => {
  it("answers JSON with a short public cache", async () => {
    installFakeCms();
    const response = await loader({ request: req("/api/blog/search?q=news") });
    expect(response.status).toBe(200);
    expect(response.headers.get("Cache-Control")).toBe("public, max-age=60");
    expect(await response.json()).toMatchObject({ query: "news", total: 9 });
  });

  it("answers an uncached 503 without upstream details when the CMS is not configured", async () => {
    installFakeCms();
    vi.stubEnv("BLOG_CMS_URL", "");
    const response = await loader({ request: req("/api/blog/search?q=news") });
    expect(response.status).toBe(503);
    expect(response.headers.get("Cache-Control")).toBe("no-store");
    expect(response.headers.get("Retry-After")).toBe("300");
    expect(await response.json()).toEqual({ error: "Blog search unavailable" });
  });
});
