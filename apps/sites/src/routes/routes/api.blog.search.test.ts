import { afterEach, describe, expect, it, vi } from "vitest";

import { installFakeCms } from "@/data/lib/blog/cms.test-utils";

import { loader } from "./api.blog.search";

afterEach(() => {
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
});

const req = () => new Request("https://sites.test/api/blog/search?q=news");

describe("GET /api/blog/search", () => {
  it("answers JSON with a short public cache", async () => {
    installFakeCms();
    const response = await loader({ request: req() });
    expect(response.status).toBe(200);
    expect(response.headers.get("Cache-Control")).toBe("public, max-age=60");
    expect(await response.json()).toMatchObject({ query: "news", total: 9 });
  });

  it("answers an uncached 503 without upstream details when the CMS is not configured", async () => {
    installFakeCms();
    vi.stubEnv("BLOG_CMS_URL", "");
    const response = await loader({ request: req() });
    expect(response.status).toBe(503);
    expect(response.headers.get("Cache-Control")).toBe("no-store");
    expect(response.headers.get("Retry-After")).toBe("300");
    expect(await response.json()).toEqual({ error: "Blog search unavailable" });
  });
});
