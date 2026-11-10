// The blog's one data path (blog plan R1-1): route loaders and browser continuation call the same
// BlogSource. Which upstream backs it (owner decision O1) is not decided yet, so the production
// source is unbound and every read fails with BlogSourceUnavailable, which routes answer as a generic
// 503. Tests and local dev pass a fixture source explicitly.
import type { BlogListScope } from "./keys";
import type {
  BlogAuthor,
  BlogCategory,
  BlogPage,
  BlogPageParam,
  BlogPost,
} from "./types";

export type BlogSource = {
  /** One page of a home, category or author list. */
  listPosts(
    scope: BlogListScope,
    param: BlogPageParam,
    signal: AbortSignal,
  ): Promise<BlogPage>;
  /** One page of search results; each post carries its highlight markup (see mapSearchPage). */
  searchPosts(
    q: string,
    param: BlogPageParam,
    signal: AbortSignal,
  ): Promise<BlogPage>;
  /** A post by its category and post slugs, or null when there is none. */
  getPost(
    categorySlug: string,
    postSlug: string,
    signal: AbortSignal,
  ): Promise<BlogPost | null>;
  /** Every category (the sub-nav shows the ones with isShownInMenu). */
  listCategories(signal: AbortSignal): Promise<BlogCategory[]>;
  /** An author by slug, or null when there is none. */
  getAuthor(slug: string, signal: AbortSignal): Promise<BlogAuthor | null>;
};

/** The blog has no configured upstream. Routes answer it with a generic 503. */
export class BlogSourceUnavailable extends Error {
  constructor() {
    super("The blog is not available.");
    this.name = "BlogSourceUnavailable";
  }
}

const unavailable = async (): Promise<never> => {
  throw new BlogSourceUnavailable();
};

/** The production source: unbound until O1 is decided. */
export const unboundBlogSource: BlogSource = {
  listPosts: unavailable,
  searchPosts: unavailable,
  getPost: unavailable,
  listCategories: unavailable,
  getAuthor: unavailable,
};

/** A generic 503 for an unbound or unreachable source: no upstream names, URLs or configuration. */
export const blogUnavailableResponse = () =>
  new Response("The blog is temporarily unavailable.", {
    status: 503,
    headers: {
      "Content-Type": "text/plain; charset=utf-8",
      "Cache-Control": "no-store",
      "Retry-After": "300",
    },
  });
