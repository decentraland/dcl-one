import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { installFakeCms } from "@/data/lib/blog/cms.test-utils";

import { loader } from "./api.blog.search";

const req = () => new Request("https://sites.test/api/blog/search?q=news");

describe("GET /api/blog/search", () => {
  let response: Response;

  beforeEach(() => {
    installFakeCms();
  });

  afterEach(() => {
    vi.unstubAllEnvs();
    vi.unstubAllGlobals();
  });

  describe("when the CMS is configured", () => {
    beforeEach(async () => {
      response = await loader({ request: req() });
    });

    it("should answer 200", () => {
      expect(response.status).toBe(200);
    });

    it("should set a short public cache", () => {
      expect(response.headers.get("Cache-Control")).toBe("public, max-age=60");
    });

    it("should answer the search results as JSON", async () => {
      expect(await response.json()).toMatchObject({ query: "news", total: 9 });
    });
  });

  describe("when the CMS is not configured", () => {
    beforeEach(async () => {
      vi.stubEnv("BLOG_CMS_URL", "");
      response = await loader({ request: req() });
    });

    it("should answer 503", () => {
      expect(response.status).toBe(503);
    });

    it("should not be cached and ask to retry later", () => {
      expect(response.headers.get("Cache-Control")).toBe("no-store");
      expect(response.headers.get("Retry-After")).toBe("300");
    });

    it("should answer an error without upstream details", async () => {
      expect(await response.json()).toEqual({ error: "Blog search unavailable" });
    });
  });
});
