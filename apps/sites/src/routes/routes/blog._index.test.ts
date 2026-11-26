import { RouterContextProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { CMS_BASE, installFakeCms } from "@/data/lib/blog/cms.test-utils";
import { BLOG_DEFAULT_DESCRIPTION, BLOG_DEFAULT_IMAGE, blogHeaders } from "../lib/blog";
import { notFoundMeta } from "../lib/social-meta";

import { loader as homeLoader, meta } from "./blog._index";
import { loader as categoryLoader, meta as categoryMeta } from "./blog.$category";
import { loader as postLoader, meta as postMeta } from "./blog.$category_.$post";
import { meta as authorMeta } from "./blog.author.$author";
import { loader as searchLoader, meta as searchMeta } from "./blog.search";

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

type Tags = ReturnType<typeof meta>;

const BASE = "https://public.test";
const root = { id: "root", loaderData: { publicPageBase: BASE } };
const content = (tags: Tags, key: string) =>
  tags.filter((t) => ("property" in t && t.property === key) || ("name" in t && t.name === key)).map((t) => (t as { content: string }).content);
const titleOf = (tags: Tags) => (tags.find((t) => "title" in t) as { title: string } | undefined)?.title;
const canonicalOf = (tags: Tags) => (tags.find((t) => "rel" in t && t.rel === "canonical") as { href: string } | undefined)?.href;
const jsonLdOf = (tags: Tags) => tags.filter((t) => "script:ld+json" in t).map((t) => JSON.parse(JSON.stringify((t as Record<string, unknown>)["script:ld+json"])));

const card = (overrides: Record<string, unknown> = {}) => ({
  slug: "first-post",
  title: "First post",
  description: "The first one",
  publishedAt: "2026-07-02",
  readMinutes: 3,
  category: { slug: "platform", title: "Platform" },
  author: { slug: "jane", name: "Jane", image: "" },
  image: { url: "https://images.test/first.png", width: 1200, height: 630, alt: "" },
  hue: 10,
  ...overrides,
});

const postData = (overrides: Record<string, unknown> = {}) => ({
  state: "ok",
  post: { ...card(overrides), body: null, assets: {} },
  related: null,
  shareUrl: `${BASE}/blog/platform/first-post`,
  categories: [],
});

const listData = (overrides: Record<string, unknown> = {}) => ({
  state: "ok",
  kind: "home",
  viewKey: "home",
  title: "The Decentraland Blog",
  subtitle: "Stories",
  activeCategory: "",
  query: "",
  posts: [card(), card({ slug: "second", title: "Second", publishedAt: "", category: { slug: "events", title: "Events" } })],
  total: 2,
  nextHref: null,
  emptyText: "",
  categories: [],
  ...overrides,
});

const postArgs = (loaderData: unknown, pathname = "/blog/platform/first-post", matches: unknown[] = [root]) =>
  ({ loaderData, matches, location: { pathname, search: "?utm_source=x" } }) as never;

describe("blog meta", () => {
  describe("when the page is the home", () => {
    describe("and the CMS answered posts", () => {
      let tags: Tags;

      beforeEach(() => {
        tags = meta({ loaderData: listData(), matches: [root], location: { search: "" } } as never);
      });

      it("should title it as the blog with the site suffix", () => {
        expect(titleOf(tags)).toBe("Decentraland Blog | Decentraland");
        expect(content(tags, "og:title")).toEqual(["Decentraland Blog | Decentraland"]);
      });

      it("should describe and picture it with the newest post", () => {
        expect(content(tags, "description")).toEqual(["The first one"]);
        expect(content(tags, "og:image")).toEqual(["https://images.test/first.png"]);
        expect(content(tags, "twitter:card")).toEqual(["summary_large_image"]);
        expect(content(tags, "og:type")).toEqual(["website"]);
      });

      it("should point the canonical and og:url at the blog", () => {
        expect(canonicalOf(tags)).toBe(`${BASE}/blog`);
        expect(content(tags, "og:url")).toEqual([`${BASE}/blog`]);
      });

      it("should list the posts as a schema.org Blog", () => {
        expect(jsonLdOf(tags)).toEqual([
          {
            "@context": "https://schema.org",
            "@type": "Blog",
            name: "Decentraland Blog",
            url: `${BASE}/blog`,
            blogPost: [
              { "@type": "BlogPosting", headline: "First post", datePublished: "2026-07-02", url: `${BASE}/blog/platform/first-post` },
              { "@type": "BlogPosting", headline: "Second", url: `${BASE}/blog/events/second` },
            ],
          },
        ]);
      });
    });

    describe("and the CMS answered no posts", () => {
      let tags: Tags;

      beforeEach(() => {
        tags = meta({ loaderData: listData({ posts: [], total: 0 }), matches: [root], location: { search: "" } } as never);
      });

      it("should use the blog's default description and picture", () => {
        expect(content(tags, "description")).toEqual([BLOG_DEFAULT_DESCRIPTION]);
        expect(content(tags, "og:image")).toEqual([BLOG_DEFAULT_IMAGE]);
      });

      it("should answer an empty Blog", () => {
        expect(jsonLdOf(tags)[0]).toMatchObject({ "@type": "Blog", blogPost: [] });
      });
    });

    describe("and the CMS is unavailable", () => {
      let tags: Tags;

      beforeEach(() => {
        tags = meta({ loaderData: { state: "unavailable" }, matches: [root], location: { search: "" } } as never);
      });

      it("should answer the blog's default card", () => {
        expect(titleOf(tags)).toBe("Decentraland Blog");
        expect(content(tags, "og:description")).toEqual([BLOG_DEFAULT_DESCRIPTION]);
        expect(content(tags, "og:image")).toEqual([BLOG_DEFAULT_IMAGE]);
        expect(canonicalOf(tags)).toBe(`${BASE}/blog`);
        expect(jsonLdOf(tags)).toEqual([]);
      });
    });

    describe.each([
      ["?page=2&utm_campaign=test", "/blog?page=2"],
      ["?page=1", "/blog"],
      ["?page=invalid", "/blog"],
    ])("and the search is %s", (search, expected) => {
      it("should use the displayed page in the canonical", () => {
        expect(canonicalOf(meta({ loaderData: listData(), matches: [root], location: { search } } as never))).toBe(BASE + expected);
      });
    });

    describe("and the root loader failed", () => {
      let tags: Tags;

      beforeEach(() => {
        tags = meta({ loaderData: listData(), matches: [], location: { search: "" } } as never);
      });

      it("should omit the canonical, og:url and every URL in the Blog", () => {
        expect(canonicalOf(tags)).toBeUndefined();
        expect(content(tags, "og:url")).toEqual([]);
        expect(JSON.stringify(jsonLdOf(tags))).not.toContain('"url"');
      });
    });
  });

  describe("when the page is a category", () => {
    describe("and it loaded", () => {
      let tags: Tags;

      beforeEach(() => {
        tags = categoryMeta({ loaderData: listData({ kind: "category", title: "Platform", subtitle: "About" }) } as never);
      });

      it("should title and describe it after the category", () => {
        expect(titleOf(tags)).toBe("Platform | Decentraland");
        expect(content(tags, "og:description")).toEqual(["About"]);
        expect(content(tags, "og:image")).toEqual([BLOG_DEFAULT_IMAGE]);
        expect(content(tags, "og:type")).toEqual(["website"]);
      });
    });

    describe("and it has no description", () => {
      it("should use the blog's default description", () => {
        expect(content(categoryMeta({ loaderData: listData({ kind: "category", title: "Platform", subtitle: "" }) } as never), "description")).toEqual([
          BLOG_DEFAULT_DESCRIPTION,
        ]);
      });
    });

    describe("and it is missing", () => {
      it("should answer the not-found head", () => {
        expect(categoryMeta({ loaderData: { state: "not-found", kind: "category" } } as never)).toEqual(notFoundMeta());
      });
    });

    describe("and the CMS is unavailable", () => {
      it("should answer the blog's default card", () => {
        const tags = categoryMeta({ loaderData: { state: "unavailable" } } as never);
        expect(titleOf(tags)).toBe("Decentraland Blog");
        expect(content(tags, "description")).toEqual([BLOG_DEFAULT_DESCRIPTION]);
      });
    });
  });

  describe("when the page is an author's", () => {
    describe("and it loaded", () => {
      let tags: Tags;

      beforeEach(() => {
        tags = authorMeta({ loaderData: listData({ kind: "author", title: "Jane", subtitle: "Writes things" }) } as never);
      });

      it("should title it as the author's posts", () => {
        expect(titleOf(tags)).toBe("Posts by Jane | Decentraland");
        expect(content(tags, "twitter:title")).toEqual(["Posts by Jane | Decentraland"]);
        expect(content(tags, "og:description")).toEqual(["Writes things"]);
      });
    });

    describe("and it is missing", () => {
      it("should answer the not-found head", () => {
        expect(authorMeta({ loaderData: { state: "not-found", kind: "author" } } as never)).toEqual(notFoundMeta());
      });
    });
  });

  describe("when the page is a search", () => {
    describe("and it has a query", () => {
      let tags: Tags;

      beforeEach(() => {
        tags = searchMeta({ location: { search: "?q=%20genesis%20" } } as never);
      });

      it("should name the query", () => {
        expect(titleOf(tags)).toBe("Search: genesis | Decentraland");
        expect(content(tags, "og:description")).toEqual(['Search results for "genesis" in Decentraland Blog']);
        expect(content(tags, "og:image")).toEqual([BLOG_DEFAULT_IMAGE]);
      });
    });

    describe("and it has no query", () => {
      let tags: Tags;

      beforeEach(() => {
        tags = searchMeta({ location: { search: "" } } as never);
      });

      it("should title it as the search", () => {
        expect(titleOf(tags)).toBe("Search | Decentraland");
        expect(content(tags, "og:description")).toEqual(["Search Decentraland Blog"]);
      });
    });
  });

  describe("when the page is a post", () => {
    describe("and it loaded with an author, date, category and picture", () => {
      let tags: Tags;

      beforeEach(() => {
        tags = postMeta(postArgs(postData(), "/blog/platform/first-post/"));
      });

      it("should title it after the post as an article", () => {
        expect(titleOf(tags)).toBe("First post | Decentraland");
        expect(content(tags, "og:title")).toEqual(["First post | Decentraland"]);
        expect(content(tags, "og:type")).toEqual(["article"]);
        expect(content(tags, "twitter:card")).toEqual(["summary_large_image"]);
      });

      it("should describe and picture it with its own values", () => {
        expect(content(tags, "og:description")).toEqual(["The first one"]);
        expect(content(tags, "og:image")).toEqual(["https://images.test/first.png"]);
      });

      it("should answer the article tags", () => {
        expect(content(tags, "article:author")).toEqual(["Jane"]);
        expect(content(tags, "article:published_time")).toEqual(["2026-07-02"]);
        expect(content(tags, "article:section")).toEqual(["Platform"]);
      });

      it("should point the canonical at the path without query or trailing slash", () => {
        expect(canonicalOf(tags)).toBe(`${BASE}/blog/platform/first-post`);
        expect(content(tags, "og:url")).toEqual([`${BASE}/blog/platform/first-post`]);
      });

      it("should answer the BlogPosting", () => {
        const url = `${BASE}/blog/platform/first-post`;
        expect(jsonLdOf(tags)).toEqual([
          {
            "@context": "https://schema.org",
            "@type": "BlogPosting",
            headline: "First post",
            url,
            mainEntityOfPage: url,
            publisher: { "@type": "Organization", name: "Decentraland", url: BASE },
            description: "The first one",
            datePublished: "2026-07-02",
            author: { "@type": "Person", name: "Jane" },
            articleSection: "Platform",
            image: "https://images.test/first.png",
          },
        ]);
      });
    });

    describe("and it has no author", () => {
      let tags: Tags;

      beforeEach(() => {
        tags = postMeta(postArgs(postData({ author: { slug: "", name: "Decentraland", image: "" } })));
      });

      it("should answer a website card without article tags", () => {
        expect(content(tags, "og:type")).toEqual(["website"]);
        expect(content(tags, "article:author")).toEqual([]);
        expect(content(tags, "article:section")).toEqual([]);
        expect(jsonLdOf(tags)[0]).not.toHaveProperty("author");
      });
    });

    describe("and it has an author but no date", () => {
      let tags: Tags;

      beforeEach(() => {
        tags = postMeta(postArgs(postData({ publishedAt: "" })));
      });

      it("should stay an article without article tags", () => {
        expect(content(tags, "og:type")).toEqual(["article"]);
        expect(content(tags, "article:author")).toEqual([]);
        expect(content(tags, "article:published_time")).toEqual([]);
        expect(jsonLdOf(tags)[0]).not.toHaveProperty("datePublished");
      });
    });

    describe("and its category is unknown", () => {
      let tags: Tags;

      beforeEach(() => {
        tags = postMeta(postArgs(postData({ category: { slug: "uncategorized", title: "Uncategorized" } })));
      });

      it("should omit the section", () => {
        expect(content(tags, "article:author")).toEqual(["Jane"]);
        expect(content(tags, "article:section")).toEqual([]);
        expect(jsonLdOf(tags)[0]).not.toHaveProperty("articleSection");
      });
    });

    describe.each([
      ["no picture", null],
      ["a picture that is not https", { url: "http://images.test/first.png", width: 0, height: 0, alt: "" }],
    ])("and it has %s", (_case, image) => {
      let tags: Tags;

      beforeEach(() => {
        tags = postMeta(postArgs(postData({ image, description: "" })));
      });

      it("should use the blog's default picture and description, and leave them out of the BlogPosting", () => {
        expect(content(tags, "og:image")).toEqual([BLOG_DEFAULT_IMAGE]);
        expect(content(tags, "og:description")).toEqual([BLOG_DEFAULT_DESCRIPTION]);
        expect(jsonLdOf(tags)[0]).not.toHaveProperty("image");
        expect(jsonLdOf(tags)[0]).not.toHaveProperty("description");
      });
    });

    describe("and it is missing", () => {
      it("should answer the not-found head", () => {
        expect(postMeta(postArgs({ state: "not-found", kind: "post" }))).toEqual(notFoundMeta());
      });
    });

    describe("and the blog is unavailable", () => {
      let tags: Tags;

      beforeEach(() => {
        tags = postMeta(postArgs({ state: "unavailable" }));
      });

      it("should answer the blog's default card at the post's canonical", () => {
        expect(titleOf(tags)).toBe("Decentraland Blog");
        expect(content(tags, "og:image")).toEqual([BLOG_DEFAULT_IMAGE]);
        expect(content(tags, "og:type")).toEqual(["website"]);
        expect(canonicalOf(tags)).toBe(`${BASE}/blog/platform/first-post`);
        expect(jsonLdOf(tags)).toEqual([]);
      });
    });

    describe("and the root loader failed", () => {
      let tags: Tags;

      beforeEach(() => {
        tags = postMeta(postArgs(postData(), "/blog/platform/first-post", []));
      });

      it("should omit the canonical and every URL in the BlogPosting", () => {
        expect(canonicalOf(tags)).toBeUndefined();
        expect(content(tags, "og:url")).toEqual([]);
        expect(jsonLdOf(tags)[0]).toEqual(expect.not.objectContaining({ url: expect.anything(), mainEntityOfPage: expect.anything() }));
        expect(jsonLdOf(tags)[0].publisher).toEqual({ "@type": "Organization", name: "Decentraland" });
      });
    });
  });
});
