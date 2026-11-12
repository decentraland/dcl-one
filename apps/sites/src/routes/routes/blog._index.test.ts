import { RouterContextProvider } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import { CMS_BASE, installFakeCms } from "@/data/lib/blog/cms.test-utils";
import { blogHeaders } from "../lib/blog";

import { loader as homeLoader, meta } from "./blog._index";
import { loader as categoryLoader, meta as categoryMeta } from "./blog.$category";
import { loader as postLoader, meta as postMeta } from "./blog.$category_.$post";
import { loader as searchLoader } from "./blog.search";

type Result = { data: Record<string, unknown>; init: ResponseInit | null };

const call = async (loader: (args: never) => unknown, path: string, params: Record<string, string> = {}) =>
  (await loader({ request: new Request(`https://sites.test${path}`), context: new RouterContextProvider(), params } as never)) as Result;

afterEach(() => {
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
});

describe("blog routes", () => {
  it("answer 200 with the CMS data and the session id", async () => {
    installFakeCms();
    const { data, init } = await call(homeLoader, "/blog");
    expect(init?.status).toBe(200);
    expect(data).toMatchObject({ state: "ok", kind: "home", total: 11 });
    expect(typeof data.sid).toBe("string");
  });

  it("answer 503 with no-store and nothing about the upstream when BLOG_CMS_URL is unset", async () => {
    vi.stubEnv("BLOG_CMS_URL", "");
    for (const [loader, path, params] of [
      [homeLoader, "/blog", {}],
      [categoryLoader, "/blog/platform", { category: "platform" }],
      [postLoader, "/blog/platform/first-post", { category: "platform", post: "first-post" }],
    ] as const) {
      const { data, init } = await call(loader, path, params);
      expect(init?.status).toBe(503);
      expect(new Headers(init?.headers).get("Cache-Control")).toBe("no-store");
      expect(data.state).toBe("unavailable");
    }
  });

  it("answer 503 without the CMS URL in the payload when the CMS fails", async () => {
    installFakeCms("status");
    const { data, init } = await call(postLoader, "/blog/platform/first-post", { category: "platform", post: "first-post" });
    expect(init?.status).toBe(503);
    expect(JSON.stringify(data)).not.toContain(CMS_BASE);
    expect(JSON.stringify(data)).not.toContain("cms.example");
  });

  it("answer 404 for an unknown category or post", async () => {
    installFakeCms();
    expect((await call(categoryLoader, "/blog/nope", { category: "nope" })).init?.status).toBe(404);
    expect((await call(postLoader, "/blog/platform/nope", { category: "platform", post: "nope" })).init?.status).toBe(404);
  });

  it("redirect an empty search to the home and a post under another category to its own URL", async () => {
    installFakeCms();
    const empty = await searchLoader({ request: new Request("https://sites.test/blog/search?q="), context: new RouterContextProvider() } as never).catch((r: unknown) => r);
    expect(empty).toBeInstanceOf(Response);
    expect((empty as Response).headers.get("Location")).toBe("/blog");
    const moved = await postLoader({ request: new Request("https://sites.test/blog/x/first-post"), context: new RouterContextProvider(), params: { category: "x", post: "first-post" } } as never).catch((r: unknown) => r);
    expect((moved as Response).status).toBe(301);
    expect((moved as Response).headers.get("Location")).toBe("/blog/platform/first-post");
  });

  it("pass a 503's Cache-Control and Retry-After to the document, and keep the parent's otherwise", () => {
    const parentHeaders = new Headers({ "Cache-Control": "private", Vary: "Cookie" });
    const failed = blogHeaders({ loaderHeaders: new Headers({ "Cache-Control": "no-store", "Retry-After": "300" }), parentHeaders } as never);
    expect(failed.get("Cache-Control")).toBe("no-store");
    expect(failed.get("Retry-After")).toBe("300");
    expect(failed.get("Vary")).toBe("Cookie");
    const ok = blogHeaders({ loaderHeaders: new Headers(), parentHeaders } as never);
    expect(ok.get("Cache-Control")).toBe("private");
  });

  it("title each page, including a missing or unavailable one", () => {
    expect(meta()).toEqual([
      { title: "Decentraland Blog" },
      { name: "description", content: "News, creator stories, events, and updates from the Decentraland community." },
    ]);
    expect(categoryMeta({ loaderData: { state: "ok", title: "Platform", subtitle: "About" } } as never)).toEqual([
      { title: "Platform | Decentraland Blog" },
      { name: "description", content: "About" },
    ]);
    expect(categoryMeta({ loaderData: { state: "not-found", kind: "category" } } as never)).toEqual([{ title: "Category not found | Decentraland Blog" }]);
    expect(postMeta({ loaderData: { state: "ok", post: { title: "T", description: "D" } } } as never)).toEqual([
      { title: "T | Decentraland Blog" },
      { name: "description", content: "D" },
    ]);
    expect(postMeta({ loaderData: { state: "not-found", kind: "post" } } as never)).toEqual([{ title: "Post not found | Decentraland Blog" }]);
    expect(postMeta({ loaderData: { state: "unavailable" } } as never)).toEqual([{ title: "Blog unavailable | Decentraland Blog" }]);
  });
});
