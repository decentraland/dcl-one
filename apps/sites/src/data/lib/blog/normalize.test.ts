import { describe, expect, it } from "vitest";

import {
  asCmsList,
  bodyAssetIds,
  decodeHtmlEntities,
  mapAsset,
  mapAuthor,
  mapCategory,
  mapPost,
  MAX_PAGES,
  parseBlogPage,
  parseBlogQuery,
  publishedAt,
  readMinutes,
  slugify,
} from "./normalize";
import { FIRST_POST_BODY } from "./cms.test-utils";

const lookups = {
  categories: new Map([["c1", { id: "c1", slug: "platform", title: "Platform", description: "", isShownInMenu: true }]]),
  authors: new Map([["a1", { id: "a1", slug: "jane-doe", name: "Jane Doe", description: "", imageId: "face" }]]),
  assets: new Map([
    ["hero", { url: "https://img.example/hero.png", width: 1200, height: 600, alt: "Hero" }],
    ["face", { url: "https://img.example/face.png", width: 200, height: 200, alt: "" }],
  ]),
};

const entry = (fields: Record<string, unknown>) => ({ sys: { id: "e1" }, fields });

describe("mapPost", () => {
  it("maps a post with its category, author, picture, UTC date and read time", () => {
    const post = mapPost(
      entry({
        id: "my-post",
        title: "Q&amp;A night",
        description: "Short &quot;intro&quot;",
        publishedDate: "2026-10-05T20:00-07:00",
        category: { sys: { id: "c1" } },
        author: { sys: { id: "a1" } },
        image: { sys: { id: "hero" } },
        body: FIRST_POST_BODY,
      }),
      lookups,
    );
    expect(post).toMatchObject({
      id: "e1",
      slug: "my-post",
      title: "Q&A night",
      description: 'Short "intro"',
      publishedAt: "2026-10-06",
      readMinutes: 1,
      category: { slug: "platform", title: "Platform" },
      author: { slug: "jane-doe", name: "Jane Doe", image: "https://img.example/face.png" },
      image: { url: "https://img.example/hero.png", alt: "Hero" },
    });
    expect(post?.body).toBe(FIRST_POST_BODY);
  });

  it("falls back to Uncategorized, the Decentraland author and no picture when links do not resolve", () => {
    const post = mapPost(entry({ id: "x", title: "X", category: { sys: { id: "nope" } }, image: { sys: { id: "missing" } } }), lookups);
    expect(post).toMatchObject({
      category: { slug: "uncategorized", title: "Uncategorized" },
      author: { slug: "", name: "Decentraland", image: "" },
      image: null,
      publishedAt: "",
      readMinutes: 1,
      body: null,
    });
  });

  it("drops an entry without a slug or a title", () => {
    expect(mapPost(entry({ title: "No slug" }), lookups)).toBeNull();
    expect(mapPost(entry({ id: "no-title" }), lookups)).toBeNull();
    expect(mapPost({ sys: { id: "" }, fields: { id: "a", title: "A" } }, lookups)).toBeNull();
  });
});

describe("categories, authors and assets", () => {
  it("slugs a category by its id field, else its raw title, and keeps the menu flag", () => {
    expect(mapCategory(entry({ id: "news", title: "News", isShownInMenu: false }))).toMatchObject({ slug: "news", isShownInMenu: false });
    expect(mapCategory(entry({ title: "Q&amp;A" }))).toMatchObject({ slug: "qampa", title: "Q&A", isShownInMenu: true });
    expect(mapCategory(entry({}))).toBeNull();
  });

  it("slugs an author by id, title or entry id and decodes the name", () => {
    expect(mapAuthor(entry({ title: "Sam &amp; Co" }))).toMatchObject({ slug: "sam-amp-co", name: "Sam & Co", imageId: "" });
    expect(mapAuthor(entry({}))).toMatchObject({ slug: "e1", name: "Decentraland" });
  });

  it("reads an asset's file as https with its size and title, and nothing without a URL", () => {
    expect(mapAsset({ sys: { id: "a" }, fields: { title: "T", file: { url: "//img.example/a.png", details: { image: { width: 10, height: 5 } } } } })).toEqual({
      url: "https://img.example/a.png",
      width: 10,
      height: 5,
      alt: "T",
    });
    expect(mapAsset({ sys: { id: "a" }, fields: { file: {} } })).toBeNull();
    expect(mapAsset(null)).toBeNull();
  });

  it("finds the assets a body embeds", () => {
    expect(bodyAssetIds(FIRST_POST_BODY)).toEqual(["body-image"]);
    expect(bodyAssetIds(null)).toEqual([]);
  });
});

describe("parsers", () => {
  it("trims and caps the search; whitespace is no search", () => {
    expect(parseBlogQuery("  genesis  ")).toBe("genesis");
    expect(parseBlogQuery("   ")).toBe("");
    expect(parseBlogQuery(null)).toBe("");
    expect(parseBlogQuery("x".repeat(500))).toHaveLength(200);
  });

  it("reads ?page= as 1..MAX_PAGES", () => {
    expect(parseBlogPage("3")).toBe(3);
    for (const bad of [null, "", "0", "-2", "1.5", "abc"]) expect(parseBlogPage(bad)).toBe(1);
    expect(parseBlogPage("9999")).toBe(MAX_PAGES);
  });

  it("dates in UTC, reads date-only values, and gives up on garbage", () => {
    expect(publishedAt("2026-07-02")).toBe("2026-07-02");
    expect(publishedAt("2026-07-02T23:30-03:00")).toBe("2026-07-03");
    expect(publishedAt("soon")).toBe("");
    expect(publishedAt(42)).toBe("");
  });

  it("reads 200 words a minute, at least one", () => {
    const words = (n: number) => ({ nodeType: "document", content: [{ nodeType: "text", value: "word ".repeat(n) }] });
    expect(readMinutes(words(401))).toBe(3);
    expect(readMinutes(null)).toBe(1);
  });

  it("slugifies and decodes as decentraland.org", () => {
    expect(slugify("  Hello, World_again ")).toBe("hello-world-again");
    expect(decodeHtmlEntities("&amp;lt; &nbsp;")).toBe("&lt;  ");
  });

  it("accepts only a list shape", () => {
    expect(asCmsList({ items: [{ sys: { id: "a" } }, null, { nope: 1 }], total: 9 })).toEqual({ items: [{ sys: { id: "a" } }], total: 9 });
    expect(asCmsList({ items: "x" })).toBeNull();
    expect(asCmsList(null)).toBeNull();
  });
});
