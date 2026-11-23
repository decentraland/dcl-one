import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { installFakeCms, requestedUrls } from "./cms.test-utils";
import { BLOG_LOADER_TIMEOUT_MS, loadBlogSearch, loadBlogSearchSuggestions } from "./pages";

const req = (path: string, init?: RequestInit) => new Request(`https://sites.test${path}`, init);
const searches = (fetchMock: Parameters<typeof requestedUrls>[0]) =>
  requestedUrls(fetchMock).filter((u) => u.includes("/blog/posts") && u.includes("q="));

type SuggestionsPayload = { query: string; total: number; posts: { slug: string; href: string; title: string; description: string }[] };

describe("loadBlogSearchSuggestions", () => {
  afterEach(() => {
    vi.resetAllMocks();
    vi.unstubAllEnvs();
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  describe("when the query matches posts", () => {
    let outcome: Awaited<ReturnType<typeof loadBlogSearchSuggestions>>;
    let data: SuggestionsPayload;

    beforeEach(async () => {
      installFakeCms();
      outcome = await loadBlogSearchSuggestions(req("/api/blog/search?q=%20News%20"));
      if (outcome.status !== 200 || !("payload" in outcome)) throw new Error("expected a payload");
      data = outcome.payload as SuggestionsPayload;
    });

    it("should answer with a 200 status", () => {
      expect(outcome.status).toBe(200);
    });

    it("should answer the trimmed query", () => {
      expect(data.query).toBe("News");
    });

    it("should answer the full search's total", () => {
      expect(data.total).toBe(9);
    });

    it("should answer the first 4 matches", () => {
      expect(data.posts).toHaveLength(4);
    });

    it("should answer each match with its URL and description, without its body", () => {
      for (const post of data.posts) {
        expect(post.href).toMatch(new RegExp(`^/blog/[a-z-]+/${post.slug}$`));
        expect(post.description).toBe(`${post.title} in short`);
        expect(post).not.toHaveProperty("body");
      }
    });
  });

  describe("when the query is under 3 characters or blank", () => {
    let fetchMock: ReturnType<typeof installFakeCms>;

    beforeEach(() => {
      fetchMock = installFakeCms();
    });

    it.each(["", "%20%20", "ab", "%20ab%20"])("should answer no posts for %j", async (q) => {
      const outcome = await loadBlogSearchSuggestions(req(`/api/blog/search?q=${q}`));
      expect(outcome).toMatchObject({ status: 200, payload: { posts: [], total: 0 } });
    });

    it("should ask the CMS nothing", async () => {
      for (const q of ["", "%20%20", "ab", "%20ab%20"]) await loadBlogSearchSuggestions(req(`/api/blog/search?q=${q}`));
      expect(fetchMock).not.toHaveBeenCalled();
    });
  });

  describe("when the full search follows the suggestions", () => {
    let fetchMock: ReturnType<typeof installFakeCms>;

    beforeEach(async () => {
      fetchMock = installFakeCms();
      await loadBlogSearchSuggestions(req("/api/blog/search?q=news"));
      await loadBlogSearch(req("/blog/search?q=NEWS"));
    });

    it("should share the results page's search and ask the CMS once", () => {
      expect(searches(fetchMock)).toHaveLength(1);
    });
  });

  describe.each(["status", "json", "network"] as const)("when the CMS fails (%s)", (fail) => {
    beforeEach(() => {
      installFakeCms(fail);
    });

    it("should be unavailable", async () => {
      expect(await loadBlogSearchSuggestions(req("/api/blog/search?q=news"))).toEqual({ status: 503, payload: { state: "unavailable" } });
    });
  });

  describe("when the CMS does not answer before the deadline", () => {
    let pending: ReturnType<typeof loadBlogSearchSuggestions>;

    beforeEach(async () => {
      vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
      installFakeCms("hang");
      pending = loadBlogSearchSuggestions(req("/api/blog/search?q=news"));
      await vi.advanceTimersByTimeAsync(BLOG_LOADER_TIMEOUT_MS);
    });

    it("should be unavailable", async () => {
      expect(await pending).toEqual({ status: 503, payload: { state: "unavailable" } });
    });
  });
});
