import { describe, expect, it } from "vitest";

import type { BlogArticle } from "@dcl/one-ui";
import type { BlogListData } from "@/data/lib/blog/types";

import { blogListPageView, blogPostPageView } from "./page-view";

const list = (over: Partial<BlogListData>): BlogListData => ({
  state: "ok",
  kind: "home",
  viewKey: "home",
  title: "The Decentraland Blog",
  subtitle: "",
  activeCategory: "",
  authorSlug: null,
  query: "",
  posts: [],
  total: 0,
  nextHref: null,
  emptyText: "",
  categories: [],
  ...over,
});

const post: BlogArticle = {
  slug: "genesis-plaza",
  title: "Genesis Plaza reopens",
  description: "",
  publishedAt: "2026-09-04",
  readMinutes: 3,
  category: { slug: "announcements", title: "Announcements" },
  author: { slug: "dcl-team", name: "DCL Team", image: "" },
  image: null,
  hue: 0,
  body: null,
  assets: {},
};

describe("blog page views", () => {
  it("names the home 'Blog' with its title and path", () => {
    expect(blogListPageView(list({}))).toEqual({ name: "Blog", properties: { title: "Blog", path: "/blog" } });
  });

  it("names a search with a query 'Blog Search Results', with the query's length", () => {
    expect(blogListPageView(list({ kind: "search", viewKey: null, query: "plaza" }))).toEqual({
      name: "Blog Search Results",
      properties: { title: "Search: plaza", hasQuery: true, queryLength: 5 },
    });
  });

  it("names a search without a query 'Blog Search'", () => {
    expect(blogListPageView(list({ kind: "search", viewKey: null, query: "" }))).toEqual({
      name: "Blog Search",
      properties: { title: "Search", hasQuery: false, queryLength: 0 },
    });
  });

  it("names an author's page 'Posts by {author}' with the author and slug", () => {
    expect(blogListPageView(list({ kind: "author", viewKey: "author:jane-doe", title: "Jane Doe", activeCategory: null, authorSlug: "jane-doe" }))).toEqual({
      name: "Posts by Jane Doe",
      properties: { title: "Posts by Jane Doe", author: "Jane Doe", authorSlug: "jane-doe" },
    });
  });

  it("names a category by its title, with its slug", () => {
    expect(blogListPageView(list({ kind: "category", viewKey: "category:events", title: "Events", activeCategory: "events" }))).toEqual({
      name: "Events",
      properties: { title: "Events", categorySlug: "events" },
    });
  });

  it("names a post by its title, with its slug, category, author and date", () => {
    const raw = "2026-11-20T15:04:05.000-03:00";
    expect(blogPostPageView({ state: "ok", post, publishedDate: raw, related: null, shareUrl: "", categories: [] })).toEqual({
      name: "Genesis Plaza reopens",
      properties: {
        title: "Genesis Plaza reopens",
        slug: "genesis-plaza",
        category: "Announcements",
        categorySlug: "announcements",
        author: "DCL Team",
        authorSlug: "dcl-team",
        // The CMS's own value, offset included, not the display date.
        publishedDate: "2026-11-20T15:04:05.000-03:00",
      },
    });
  });

  it("gives a post without an author the legacy default author's slug", () => {
    const anonymous = { ...post, author: { slug: "", name: "Decentraland", image: "" } };
    const view = blogPostPageView({ state: "ok", post: anonymous, publishedDate: "", related: null, shareUrl: "", categories: [] });
    expect(view?.properties).toMatchObject({ author: "Decentraland", authorSlug: "decentraland" });
  });

  it("sends nothing for a missing or unavailable page", () => {
    expect(blogListPageView({ state: "not-found", kind: "category" })).toBeNull();
    expect(blogListPageView({ state: "unavailable" })).toBeNull();
    expect(blogPostPageView({ state: "not-found", kind: "post" })).toBeNull();
    expect(blogPostPageView({ state: "unavailable" })).toBeNull();
  });
});
