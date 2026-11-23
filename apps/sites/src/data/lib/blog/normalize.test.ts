import { beforeEach, describe, expect, it } from "vitest";

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

const entry = (fields: Record<string, unknown>) => ({ sys: { id: "e1" }, fields });

describe("mapPost", () => {
  let lookups: Parameters<typeof mapPost>[1];

  beforeEach(() => {
    lookups = {
      categories: new Map([["c1", { id: "c1", slug: "platform", title: "Platform", description: "", isShownInMenu: true }]]),
      authors: new Map([["a1", { id: "a1", slug: "jane-doe", name: "Jane Doe", description: "", imageId: "face" }]]),
      assets: new Map([
        ["hero", { url: "https://img.example/hero.png", width: 1200, height: 600, alt: "Hero" }],
        ["face", { url: "https://img.example/face.png", width: 200, height: 200, alt: "" }],
      ]),
    };
  });

  describe("when every link resolves", () => {
    let post: ReturnType<typeof mapPost>;

    beforeEach(() => {
      post = mapPost(
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
    });

    it("should map the post with its category, author, picture, UTC date and read time", () => {
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
    });

    it("should keep the body", () => {
      expect(post?.body).toBe(FIRST_POST_BODY);
    });
  });

  describe("when the links do not resolve", () => {
    it("should fall back to Uncategorized, the Decentraland author and no picture", () => {
      expect(mapPost(entry({ id: "x", title: "X", category: { sys: { id: "nope" } }, image: { sys: { id: "missing" } } }), lookups)).toMatchObject({
        category: { slug: "uncategorized", title: "Uncategorized" },
        author: { slug: "", name: "Decentraland", image: "" },
        image: null,
        publishedAt: "",
        readMinutes: 1,
        body: null,
      });
    });
  });

  describe("when the entry has no slug", () => {
    it("should drop it", () => {
      expect(mapPost(entry({ title: "No slug" }), lookups)).toBeNull();
    });
  });

  describe("when the entry has no title", () => {
    it("should drop it", () => {
      expect(mapPost(entry({ id: "no-title" }), lookups)).toBeNull();
    });
  });

  describe("when the entry has no id", () => {
    it("should drop it", () => {
      expect(mapPost({ sys: { id: "" }, fields: { id: "a", title: "A" } }, lookups)).toBeNull();
    });
  });
});

describe("mapCategory", () => {
  describe("when the category has an id field", () => {
    it("should slug it by the id and keep the menu flag", () => {
      expect(mapCategory(entry({ id: "news", title: "News", isShownInMenu: false }))).toMatchObject({ slug: "news", isShownInMenu: false });
    });
  });

  describe("when the category has only a title", () => {
    it("should slug it by its raw title, decode the title and show it in the menu", () => {
      expect(mapCategory(entry({ title: "Q&amp;A" }))).toMatchObject({ slug: "qampa", title: "Q&A", isShownInMenu: true });
    });
  });

  describe("when the category has neither", () => {
    it("should drop it", () => {
      expect(mapCategory(entry({}))).toBeNull();
    });
  });
});

describe("mapAuthor", () => {
  describe("when the author has a title", () => {
    it("should slug it by the title and decode the name", () => {
      expect(mapAuthor(entry({ title: "Sam &amp; Co" }))).toMatchObject({ slug: "sam-amp-co", name: "Sam & Co", imageId: "" });
    });
  });

  describe("when the author has no id or title", () => {
    it("should slug it by the entry id and name it Decentraland", () => {
      expect(mapAuthor(entry({}))).toMatchObject({ slug: "e1", name: "Decentraland" });
    });
  });
});

describe("mapAsset", () => {
  describe("when the asset has a file URL", () => {
    it("should read it as https with its size and title", () => {
      expect(mapAsset({ sys: { id: "a" }, fields: { title: "T", file: { url: "//img.example/a.png", details: { image: { width: 10, height: 5 } } } } })).toEqual({
        url: "https://img.example/a.png",
        width: 10,
        height: 5,
        alt: "T",
      });
    });
  });

  describe("when the file has no URL", () => {
    it("should read nothing", () => {
      expect(mapAsset({ sys: { id: "a" }, fields: { file: {} } })).toBeNull();
    });
  });

  describe("when there is no asset", () => {
    it("should read nothing", () => {
      expect(mapAsset(null)).toBeNull();
    });
  });
});

describe("bodyAssetIds", () => {
  describe("when the body embeds assets", () => {
    it("should find them", () => {
      expect(bodyAssetIds(FIRST_POST_BODY)).toEqual(["body-image"]);
    });
  });

  describe("when there is no body", () => {
    it("should find none", () => {
      expect(bodyAssetIds(null)).toEqual([]);
    });
  });
});

describe("parseBlogQuery", () => {
  it("should trim the search", () => {
    expect(parseBlogQuery("  genesis  ")).toBe("genesis");
  });

  describe("when the search is only whitespace", () => {
    it("should read no search", () => {
      expect(parseBlogQuery("   ")).toBe("");
    });
  });

  describe("when there is no search", () => {
    it("should read no search", () => {
      expect(parseBlogQuery(null)).toBe("");
    });
  });

  describe("when the search is too long", () => {
    it("should cap it at 200 characters", () => {
      expect(parseBlogQuery("x".repeat(500))).toHaveLength(200);
    });
  });
});

describe("parseBlogPage", () => {
  describe("when the page is valid", () => {
    it("should read it", () => {
      expect(parseBlogPage("3")).toBe(3);
    });
  });

  describe("when the page is invalid", () => {
    it.each([null, "", "0", "-2", "1.5", "abc"])("should read %j as the first page", (bad) => {
      expect(parseBlogPage(bad)).toBe(1);
    });
  });

  describe("when the page is past MAX_PAGES", () => {
    it("should cap it", () => {
      expect(parseBlogPage("9999")).toBe(MAX_PAGES);
    });
  });
});

describe("publishedAt", () => {
  describe("when the value is date-only", () => {
    it("should read it", () => {
      expect(publishedAt("2026-07-02")).toBe("2026-07-02");
    });
  });

  describe("when the value has a time zone", () => {
    it("should date it in UTC", () => {
      expect(publishedAt("2026-07-02T23:30-03:00")).toBe("2026-07-03");
    });
  });

  describe("when the value is garbage", () => {
    it("should give up on a string", () => {
      expect(publishedAt("soon")).toBe("");
    });

    it("should give up on a number", () => {
      expect(publishedAt(42)).toBe("");
    });
  });
});

describe("readMinutes", () => {
  describe("when the body has words", () => {
    let body: Parameters<typeof readMinutes>[0];

    beforeEach(() => {
      body = { nodeType: "document", content: [{ nodeType: "text", value: "word ".repeat(401) }] };
    });

    it("should read 200 words a minute", () => {
      expect(readMinutes(body)).toBe(3);
    });
  });

  describe("when there is no body", () => {
    it("should read at least one minute", () => {
      expect(readMinutes(null)).toBe(1);
    });
  });
});

describe("slugify and decodeHtmlEntities", () => {
  it("should slugify as decentraland/sites", () => {
    expect(slugify("  Hello, World_again ")).toBe("hello-world-again");
  });

  it("should decode as decentraland/sites", () => {
    expect(decodeHtmlEntities("&amp;lt; &nbsp;")).toBe("&lt;  ");
  });
});

describe("asCmsList", () => {
  describe("when the value is a list shape", () => {
    it("should keep only its entries", () => {
      expect(asCmsList({ items: [{ sys: { id: "a" } }, null, { nope: 1 }], total: 9 })).toEqual({ items: [{ sys: { id: "a" } }], total: 9 });
    });
  });

  describe("when the items are not a list", () => {
    it("should reject it", () => {
      expect(asCmsList({ items: "x" })).toBeNull();
    });
  });

  describe("when there is no value", () => {
    it("should reject it", () => {
      expect(asCmsList(null)).toBeNull();
    });
  });
});
