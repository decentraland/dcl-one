import { describe, expect, it } from "vitest";
import {
  blogDateToIso,
  blogPostCards,
  blogReadMinutes,
  parseBlogQuery,
  parseBlogSort,
  sortBlogCards,
  type BlogPostCard,
} from "./blog";

describe("blogDateToIso", () => {
  it("reads the posts' date format", () => {
    expect(blogDateToIso("JUL 2, 2026")).toBe("2026-07-02");
    expect(blogDateToIso("jun 29, 2026")).toBe("2026-06-29");
  });

  it("returns an empty string for anything else", () => {
    for (const v of ["", "JULY 2, 2026", "XYZ 2, 2026", "JUL 0, 2026", "JUL 32, 2026", "2026-07-02"]) {
      expect(blogDateToIso(v)).toBe("");
    }
  });
});

describe("blogReadMinutes", () => {
  it("is at least one minute and rounds up at 200 words per minute", () => {
    expect(blogReadMinutes([])).toBe(1);
    expect(blogReadMinutes([{ type: "p", content: "word ".repeat(200) }])).toBe(1);
    expect(blogReadMinutes([{ type: "p", content: "word ".repeat(150) }, { type: "ul", items: ["a ".repeat(51)] }])).toBe(2);
  });
});

describe("params", () => {
  it("only 'oldest' changes the sort", () => {
    expect(parseBlogSort("oldest")).toBe("oldest");
    for (const v of [null, undefined, "", "newest", "OLDEST", "x"]) expect(parseBlogSort(v)).toBe("newest");
  });

  it("trims the query and treats a missing one as empty", () => {
    expect(parseBlogQuery("  web  ")).toBe("web");
    expect(parseBlogQuery(null)).toBe("");
  });
});

describe("sortBlogCards", () => {
  const card = (slug: string, publishedAt: string) => ({ slug, publishedAt }) as BlogPostCard;

  it("puts undated cards last in data order, whatever the direction", () => {
    const cards = [card("u1", ""), card("a", "2026-01-01"), card("u2", ""), card("b", "2026-02-01")];
    expect(sortBlogCards(cards, "newest").map((c) => c.slug)).toEqual(["b", "a", "u1", "u2"]);
    expect(sortBlogCards(cards, "oldest").map((c) => c.slug)).toEqual(["a", "b", "u1", "u2"]);
  });

  it("keeps the cards the posts expose", () => {
    const [first] = blogPostCards();
    expect(first).toMatchObject({ publishedAt: "2026-07-02", readMinutes: 1, author: "Catalyrst", category: { slug: "platform" } });
  });
});
