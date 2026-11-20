import { RouterContextProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { CMS_BASE, installFakeCms } from "@/data/lib/blog/cms.test-utils";
import { blogHeaders } from "../lib/blog";

import { loader as homeLoader, meta } from "./blog._index";
import { loader as categoryLoader, meta as categoryMeta } from "./blog.$category";
import { loader as postLoader, meta as postMeta } from "./blog.$category_.$post";
import { loader as searchLoader } from "./blog.search";

type Result = { data: Record<string, unknown>; init: ResponseInit | null };

const call = async (loader: (args: never) => unknown, path: string, params: Record<string, string> = {}) =>
  (await loader({ request: new Request(`https://sites.test${path}`), context: new RouterContextProvider(), params } as never)) as Result;

const settle = (promise: Promise<unknown>) => promise.catch((r: unknown) => r);

describe("blog routes", () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    vi.unstubAllGlobals();
  });

  describe("when the CMS answers", () => {
    beforeEach(() => {
      installFakeCms();
    });

    describe("and the home is requested", () => {
      let result: Result;

      beforeEach(async () => {
        result = await call(homeLoader, "/blog");
      });

      it("should answer 200", () => {
        expect(result.init?.status).toBe(200);
      });

      it("should answer the CMS data", () => {
        expect(result.data).toMatchObject({ state: "ok", kind: "home", total: 11 });
      });

      it("should answer the session id", () => {
        expect(typeof result.data.sid).toBe("string");
      });
    });

    describe("and the category is unknown", () => {
      let result: Result;

      beforeEach(async () => {
        result = await call(categoryLoader, "/blog/nope", { category: "nope" });
      });

      it("should answer 404", () => {
        expect(result.init?.status).toBe(404);
      });
    });

    describe("and the post is unknown", () => {
      let result: Result;

      beforeEach(async () => {
        result = await call(postLoader, "/blog/platform/nope", { category: "platform", post: "nope" });
      });

      it("should answer 404", () => {
        expect(result.init?.status).toBe(404);
      });
    });

    describe("and the search is empty", () => {
      let empty: unknown;

      beforeEach(async () => {
        empty = await settle(searchLoader({ request: new Request("https://sites.test/blog/search?q="), context: new RouterContextProvider() } as never));
      });

      it("should redirect to the home", () => {
        expect(empty).toBeInstanceOf(Response);
        expect((empty as Response).headers.get("Location")).toBe("/blog");
      });
    });

    describe("and a post is requested under another category", () => {
      let moved: unknown;

      beforeEach(async () => {
        moved = await settle(
          postLoader({ request: new Request("https://sites.test/blog/x/first-post"), context: new RouterContextProvider(), params: { category: "x", post: "first-post" } } as never),
        );
      });

      it("should redirect permanently to its own URL", () => {
        expect((moved as Response).status).toBe(301);
        expect((moved as Response).headers.get("Location")).toBe("/blog/platform/first-post");
      });
    });
  });

  describe("when BLOG_CMS_URL is unset", () => {
    beforeEach(() => {
      vi.stubEnv("BLOG_CMS_URL", "");
    });

    describe.each([
      ["home", homeLoader, "/blog", {}],
      ["category", categoryLoader, "/blog/platform", { category: "platform" }],
      ["post", postLoader, "/blog/platform/first-post", { category: "platform", post: "first-post" }],
    ] as const)("and the %s is requested", (_page, loader, path, params) => {
      let result: Result;

      beforeEach(async () => {
        result = await call(loader, path, params);
      });

      it("should answer 503", () => {
        expect(result.init?.status).toBe(503);
      });

      it("should answer with no-store", () => {
        expect(new Headers(result.init?.headers).get("Cache-Control")).toBe("no-store");
      });

      it("should say the blog is unavailable and nothing about the upstream", () => {
        expect(result.data.state).toBe("unavailable");
      });
    });
  });

  describe("when the CMS fails", () => {
    let result: Result;

    beforeEach(async () => {
      installFakeCms("status");
      result = await call(postLoader, "/blog/platform/first-post", { category: "platform", post: "first-post" });
    });

    it("should answer 503", () => {
      expect(result.init?.status).toBe(503);
    });

    it("should not put the CMS URL in the payload", () => {
      expect(JSON.stringify(result.data)).not.toContain(CMS_BASE);
      expect(JSON.stringify(result.data)).not.toContain("cms.example");
    });
  });
});

describe("blogHeaders", () => {
  let parentHeaders: Headers;

  beforeEach(() => {
    parentHeaders = new Headers({ "Cache-Control": "private", Vary: "Cookie" });
  });

  describe("when the loader answered a 503", () => {
    let failed: Headers;

    beforeEach(() => {
      failed = blogHeaders({ loaderHeaders: new Headers({ "Cache-Control": "no-store", "Retry-After": "300" }), parentHeaders } as never);
    });

    it("should pass the 503's Cache-Control and Retry-After to the document", () => {
      expect(failed.get("Cache-Control")).toBe("no-store");
      expect(failed.get("Retry-After")).toBe("300");
    });

    it("should keep the parent's other headers", () => {
      expect(failed.get("Vary")).toBe("Cookie");
    });
  });

  describe("when the loader set no headers", () => {
    let ok: Headers;

    beforeEach(() => {
      ok = blogHeaders({ loaderHeaders: new Headers(), parentHeaders } as never);
    });

    it("should keep the parent's Cache-Control", () => {
      expect(ok.get("Cache-Control")).toBe("private");
    });
  });
});

describe("blog meta", () => {
  describe("when the page is the home", () => {
    describe("and the root loader answered the public page base", () => {
      let args: Parameters<typeof meta>[0];

      beforeEach(() => {
        args = { matches: [{ loaderData: { publicPageBase: "https://public.test" } }], location: { search: "" } } as never;
      });

      it("should title it as the blog with its description and canonical", () => {
        expect(meta(args)).toEqual([
          { title: "Blog | Decentraland" },
          { name: "description", content: "News, creator stories, events, and updates from the Decentraland community." },
          { tagName: "link", rel: "canonical", href: "https://public.test/blog" },
        ]);
      });
    });

    describe.each([
      ["?page=2&utm_campaign=test", "/blog?page=2"],
      ["?page=1", "/blog"],
      ["?page=invalid", "/blog"],
    ])("and the search is %s", (search, expected) => {
      let args: Parameters<typeof meta>[0];

      beforeEach(() => {
        args = { matches: [{ loaderData: { publicPageBase: "https://public.test" } }], location: { search } } as never;
      });

      it("should use the displayed page in the canonical", () => {
        expect(meta(args)).toContainEqual({ tagName: "link", rel: "canonical", href: "https://public.test" + expected });
      });
    });

    describe("and the root loader failed", () => {
      let args: Parameters<typeof meta>[0];

      beforeEach(() => {
        args = { matches: [], location: { search: "" } } as never;
      });

      it("should omit the canonical", () => {
        expect(meta(args).some((tag) => "rel" in tag)).toBe(false);
      });
    });
  });

  describe("when the page is a category", () => {
    describe("and it loaded", () => {
      it("should title it after the category", () => {
        expect(categoryMeta({ loaderData: { state: "ok", title: "Platform", subtitle: "About" } } as never)).toEqual([
          { title: "Platform | Decentraland Blog" },
          { name: "description", content: "About" },
        ]);
      });
    });

    describe("and it is missing", () => {
      it("should title it as not found", () => {
        expect(categoryMeta({ loaderData: { state: "not-found", kind: "category" } } as never)).toEqual([{ title: "Category not found | Decentraland Blog" }]);
      });
    });
  });

  describe("when the page is a post", () => {
    describe("and it loaded", () => {
      it("should title it after the post", () => {
        expect(postMeta({ loaderData: { state: "ok", post: { title: "T", description: "D" } } } as never)).toEqual([
          { title: "T | Decentraland Blog" },
          { name: "description", content: "D" },
        ]);
      });
    });

    describe("and it is missing", () => {
      it("should title it as not found", () => {
        expect(postMeta({ loaderData: { state: "not-found", kind: "post" } } as never)).toEqual([{ title: "Post not found | Decentraland Blog" }]);
      });
    });

    describe("and the blog is unavailable", () => {
      it("should title it as unavailable", () => {
        expect(postMeta({ loaderData: { state: "unavailable" } } as never)).toEqual([{ title: "Blog unavailable | Decentraland Blog" }]);
      });
    });
  });
});
