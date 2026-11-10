import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import { blogKeys } from "./keys";
import {
  bodyAssetIds,
  decodeHtmlEntities,
  FIRST_PAGE,
  mapAuthor,
  mapCategory,
  mapPage,
  mapPost,
  mapSearchPage,
  nextPageParam,
  slugify,
  type CmsEntry,
  type CmsLookup,
} from "./normalize";
import { findUnsupported, unsupportedEntries } from "./richtext";
import type { BlogPage } from "./types";

// Public cms-api delivery responses captured on 2026-11-04 (see the fixture's capturedAt/endpoints):
// the 11 posts that together use every rich-text type found across all published posts, plus every
// category, author and referenced asset.
type Fixture = {
  posts: CmsEntry[];
  categories: CmsEntry[];
  authors: CmsEntry[];
  assets: Record<string, CmsEntry>;
};
const fixture = JSON.parse(
  fs.readFileSync(
    path.join(
      path.dirname(fileURLToPath(import.meta.url)),
      "fixtures/cms-sample.json",
    ),
    "utf8",
  ),
) as Fixture;

const lookup: CmsLookup = {
  entries: new Map(
    [...fixture.categories, ...fixture.authors].map((entry) => [
      entry.sys.id,
      entry,
    ]),
  ),
  assets: new Map(Object.entries(fixture.assets)),
};
const post = (slug: string) =>
  fixture.posts.find((entry) => entry.fields?.id === slug)!;

describe("mapPost", () => {
  describe("when a post's references resolve", () => {
    it("should carry the post, its category, author and image as source shapes them", () => {
      const mapped = mapPost(post("servers-part-3"), lookup)!;

      expect(mapped).toMatchObject({
        slug: "servers-part-3",
        title: "Servers Deep Dive Part 3: Using WebSockets",
        publishedDate: "2020-08-25T03:00:01.000Z",
        url: "/blog/tutorials/servers-part-3",
        category: {
          slug: "tutorials",
          url: "/blog/tutorials",
          isShownInMenu: false,
        },
        author: {
          slug: "nicoearnshaw",
          title: "Nico Earnshaw",
          url: "/blog/author/nicoearnshaw",
        },
      });
      expect(mapped.image.url.startsWith("https://images.ctfassets.net/")).toBe(
        true,
      );
    });
  });

  describe.each(fixture.posts.map((entry) => [entry.fields?.id as string]))(
    "when %s is mapped",
    (slug) => {
      it("should resolve every asset its body embeds or links", () => {
        const entry = post(slug);
        const mapped = mapPost(entry, lookup)!;

        expect(Object.keys(mapped.bodyAssets).sort()).toEqual(
          bodyAssetIds(mapped.body).sort(),
        );
      });
    },
  );

  describe("when the category cannot be resolved", () => {
    it("should fall back to source's Uncategorized category", () => {
      const entry = {
        ...post("servers-part-3"),
        fields: { ...post("servers-part-3").fields, category: undefined },
      };

      expect(mapPost(entry, lookup)!.category).toMatchObject({
        slug: "uncategorized",
        title: "Uncategorized",
        isShownInMenu: false,
      });
    });
  });

  describe("when the post image cannot be resolved", () => {
    it("should fall back to the Decentraland logo, sized as source", () => {
      const entry = {
        ...post("servers-part-3"),
        fields: { ...post("servers-part-3").fields, image: undefined },
      };

      expect(mapPost(entry, lookup)!.image).toMatchObject({
        url: "https://decentraland.org/logos/png/color.png",
        width: 1200,
        height: 630,
      });
    });
  });
});

describe("mapCategory", () => {
  it("should show in the menu exactly the categories production shows", () => {
    const shown = fixture.categories
      .map((entry) => mapCategory(entry, lookup))
      .filter((category) => category?.isShownInMenu)
      .map((category) => category!.slug);

    expect(shown).toEqual([
      "about-decentraland",
      "announcements",
      "community-highlights",
    ]);
  });
});

describe("mapAuthor", () => {
  it("should fall back to the Decentraland author when the entry has no fields", () => {
    expect(mapAuthor({ sys: { id: "abc" }, fields: {} }, lookup)).toMatchObject(
      { slug: "abc", title: "Decentraland", url: "/blog/author/abc" },
    );
  });
});

describe("decodeHtmlEntities", () => {
  it.each([
    ["Q&amp;A", "Q&A"],
    ["&lt;b&gt;", "<b>"],
    ["&quot;x&quot; &#39;y&apos;", "\"x\" 'y'"],
    ["a&nbsp;b", "a b"],
    ["&amp;lt;", "&lt;"],
    ["&#60;", "&#60;"],
  ])("should decode %s as source does", (input, expected) => {
    expect(decodeHtmlEntities(input)).toBe(expected);
  });
});

describe("slugify", () => {
  it.each([
    ["Community Highlights", "community-highlights"],
    ["  Q&A: The_Best -- Bits ", "qa-the-best-bits"],
    ["", ""],
  ])("should turn %j into %j", (input, expected) => {
    expect(slugify(input)).toBe(expected);
  });
});

describe("pagination", () => {
  const page = (skip: number, count: number, total: number): BlogPage => ({
    items: Array.from({ length: count }, () =>
      mapPost(post("servers-part-3"), lookup)!,
    ),
    total,
    skip,
    limit: skip === 0 ? 7 : 6,
  });

  it("should start at skip 0 with 7 posts", () => {
    expect(FIRST_PAGE).toEqual({ skip: 0, limit: 7 });
  });

  it("should continue 6 at a time from the posts already returned", () => {
    expect(nextPageParam(page(0, 7, 30))).toEqual({ skip: 7, limit: 6 });
    expect(nextPageParam(page(7, 6, 30))).toEqual({ skip: 13, limit: 6 });
  });

  it("should end when the returned posts reach the total, or on an empty page", () => {
    expect(nextPageParam(page(13, 6, 19))).toBeUndefined();
    expect(nextPageParam(page(19, 0, 30))).toBeUndefined();
  });

  it("should map a list response into one page, keeping the CMS total", () => {
    const mapped = mapPage(
      { items: [post("servers-part-3")], total: 471 },
      FIRST_PAGE,
      lookup,
    );

    expect(mapped).toMatchObject({ total: 471, skip: 0, limit: 7 });
    expect(mapped.items).toHaveLength(1);
  });
});

describe("blogKeys", () => {
  it("should key lists by scope, and search, posts and related posts by their inputs", () => {
    expect(blogKeys.list({ scope: "category", slug: "tutorials" })).toEqual([
      "blog",
      "list",
      { scope: "category", slug: "tutorials" },
    ]);
    expect(blogKeys.search("wearables")).toEqual([
      "blog",
      "search",
      { q: "wearables" },
    ]);
    expect(blogKeys.post("tutorials", "servers-part-3")).toEqual([
      "blog",
      "post",
      { category: "tutorials", slug: "servers-part-3" },
    ]);
    expect(blogKeys.related("tutorials", "servers-part-3")).toEqual([
      "blog",
      "related",
      { category: "tutorials", excludeSlug: "servers-part-3" },
    ]);
  });
});

describe("findUnsupported", () => {
  it("should find nothing unsupported in the sample but the asset links held by O5", () => {
    const found = fixture.posts.flatMap((entry) =>
      findUnsupported(mapPost(entry, lookup)!.body).nodes.map((node) => ({
        slug: entry.fields?.id,
        ...node,
      })),
    );

    expect(
      new Set(
        found.map((node) => `${node.slug}:${node.nodeType}:${node.heldBy}`),
      ),
    ).toEqual(
      new Set(["decentraland-quests-the-journey-begins:asset-hyperlink:O5"]),
    );
    expect(
      fixture.posts.every(
        (entry) =>
          findUnsupported(mapPost(entry, lookup)!.body).marks.length === 0,
      ),
    ).toBe(true);
  });

  it("should record a contentless embedded entry of an unknown type instead of dropping it", () => {
    const body = {
      nodeType: "document",
      content: [
        {
          nodeType: "paragraph",
          content: [{ nodeType: "text", value: "Before", marks: [] }],
        },
        {
          nodeType: "embedded-entry-block",
          data: {
            target: { sys: { id: "x", type: "Link", linkType: "Entry" } },
          },
          content: [],
        },
      ],
    };

    expect(findUnsupported(body).nodes).toEqual([
      { nodeType: "embedded-entry-block", path: [1], hasText: false },
    ]);
  });

  it("should record an unsupported mark with its position", () => {
    const body = {
      nodeType: "document",
      content: [
        {
          nodeType: "paragraph",
          content: [
            { nodeType: "text", value: "x", marks: [{ type: "superscript" }] },
          ],
        },
      ],
    };

    expect(findUnsupported(body).marks).toEqual([
      { mark: "superscript", path: [0, 0] },
    ]);
  });

  it("should list each unsupported node with its post slug for the dev-time report, contentless ones included", () => {
    const body = {
      nodeType: "document",
      content: [
        {
          nodeType: "embedded-entry-block",
          data: { target: { sys: { id: "x" } } },
          content: [],
        },
      ],
    };

    expect(unsupportedEntries("a-post", body)).toEqual([
      {
        nodeType: "embedded-entry-block",
        path: [0],
        hasText: false,
        slug: "a-post",
      },
    ]);
  });
});

describe("mapSearchPage", () => {
  const hit = (extra: Record<string, unknown>) => ({
    ...post("mvfw23-recap"),
    ...extra,
  });
  const param = { skip: 0, limit: 5 };

  it("keeps the CMS's highlight markup as given, never decoded or rendered", () => {
    const page = mapSearchPage(
      {
        items: [
          hit({
            _highlight: {
              title: "<em>MVFW23</em> Recap",
              description: "a &amp; <em>b</em>",
            },
          }),
        ],
        total: 1,
      },
      param,
      lookup,
    );
    expect(page.items[0]!.highlight).toEqual({
      title: "<em>MVFW23</em> Recap",
      description: "a &amp; <em>b</em>",
    });
    expect(page.items[0]!.title).toBe("MVFW23 Recap");
  });

  it("falls back to each stored field, undecoded, when a hit has no highlight for it", () => {
    const stored = post("mvfw23-recap").fields!;
    const page = mapSearchPage(
      {
        items: [hit({ _highlight: { title: "<em>x</em>" } }), hit({})],
        total: 2,
      },
      param,
      lookup,
    );
    expect(page.items[0]!.highlight).toEqual({
      title: "<em>x</em>",
      description: stored.description,
    });
    expect(page.items[1]!.highlight).toEqual({
      title: stored.title,
      description: stored.description,
    });
  });

  it("ignores a highlight that is not a string, and keeps the page counts", () => {
    const page = mapSearchPage(
      { items: [hit({ _highlight: { title: ["<em>x</em>"] } })], total: 9 },
      { skip: 5, limit: 5 },
      lookup,
    );
    expect(page.items[0]!.highlight!.title).toBe(
      post("mvfw23-recap").fields!.title,
    );
    expect([page.total, page.skip, page.limit]).toEqual([9, 5, 5]);
  });

  it("gives list pages no highlight", () => {
    expect(
      mapPage({ items: [post("mvfw23-recap")], total: 1 }, param, lookup)
        .items[0]!.highlight,
    ).toBeUndefined();
  });
});
