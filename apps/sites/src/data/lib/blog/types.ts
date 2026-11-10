import type {
  BlogArticle,
  BlogCard,
  BlogCategory,
  BlogNotFoundKind,
  BlogRelatedCard,
} from "@dcl/one-ui/apps/sites/blog";

/** A blog page the CMS could not answer: an unknown slug (404) or no reachable CMS (503). */
export type BlogMissing = { state: "not-found"; kind: BlogNotFoundKind } | { state: "unavailable" };

/** Which list a page shows; the telemetry view key, null for a search (a search keeps the view). */
export type BlogListKind = "home" | "category" | "author" | "search";

export type BlogListData = {
  state: "ok";
  kind: BlogListKind;
  /** home, `category:<slug>` or `author:<slug>`: one lp_blog_viewed per key; null for a search. */
  viewKey: string | null;
  title: string;
  subtitle: string;
  /** The tab to highlight: "" on the home, the slug on a category, null otherwise. */
  activeCategory: string | null;
  query: string;
  posts: BlogCard[];
  /** Every post the list has (the CMS total), loaded or not: what lp_blog_viewed reports. */
  total: number;
  nextHref: string | null;
  emptyText: string;
  categories: BlogCategory[];
};

export type BlogPostData = {
  state: "ok";
  post: BlogArticle;
  related: BlogRelatedCard[];
  shareUrl: string;
  categories: BlogCategory[];
};

export type BlogListPayload = BlogListData | BlogMissing;
export type BlogPostPayload = BlogPostData | BlogMissing;
