// What each blog route loads: the list pages (home, category, author, search) and a post, as a status
// and payload the route hands to its view. A CMS failure or the loader deadline is a 503; an unknown
// slug is a 404; a post reached under another category redirects to its own URL, and an old
// `/blog/<post-slug>` link (no category) redirects to the post's URL.
import {
  BLOG_HOME,
  BLOG_SEARCH,
  BLOG_SEARCH_SUGGESTIONS_LIMIT,
  blogAuthorHref,
  blogCategoryHref,
  blogPostHref,
  type BlogCategory,
  type BlogImage,
  type BlogRelatedCard,
  type BlogSearchSuggestions,
} from "@dcl/one-ui/apps/sites/blog/blogModel";

import { publicPageUrl, type BaseEnv } from "../catalyst/bases";
import { withDeadline } from "../request-deadline";
import { blogCms, BlogUnavailableError, listPageWindow, searchPageWindow, type BlogCms } from "./cms";
import {
  MAX_PAGES,
  SEARCH_MAX_PAGES,
  parseBlogPage,
  parseBlogQuery,
  SEARCH_MIN_LENGTH,
  toCard,
  type BlogCmsCategory,
  type BlogCmsPost,
} from "./normalize";
import type { BlogListData, BlogListPayload, BlogMissing, BlogPostPayload } from "./types";

/** The whole loader, every CMS read included; past it the page answers 503. */
export const BLOG_LOADER_TIMEOUT_MS = 8_000;
/** Related posts' own budget inside the loader: past it the post renders without them. */
export const BLOG_RELATED_TIMEOUT_MS = 1_500;

export type BlogOutcome<T> =
  | { status: 200; payload: T }
  | { status: 404 | 503; payload: BlogMissing }
  | { status: 301 | 302; location: string };

const unavailable = { status: 503, payload: { state: "unavailable" } } as const;
const notFound = (kind: "post" | "category" | "author") => ({ status: 404, payload: { state: "not-found", kind } }) as const;

async function guarded<T>(
  request: Request,
  env: BaseEnv | undefined,
  load: (cms: BlogCms, signal: AbortSignal) => Promise<BlogOutcome<T>>,
): Promise<BlogOutcome<T>> {
  try {
    const cms = blogCms(env);
    return await withDeadline((signal) => load(cms, signal), BLOG_LOADER_TIMEOUT_MS, request.signal);
  } catch (error) {
    request.signal.throwIfAborted();
    if (error instanceof BlogUnavailableError || (error instanceof DOMException && error.name === "TimeoutError")) return unavailable;
    throw error;
  }
}

/** The categories the tabs show (the CMS's menu flag), in CMS order. */
const menu = (categories: BlogCmsCategory[]): BlogCategory[] =>
  categories.filter((c) => c.isShownInMenu).map(({ slug, title }) => ({ slug, title }));

function nextHref(path: string, page: number, total: number, window: (n: number) => { skip: number; limit: number }, extra: Record<string, string> = {}, maxPages = MAX_PAGES): string | null {
  const { skip, limit } = window(page);
  if (page >= maxPages || skip + limit >= total) return null;
  return `${path}?${new URLSearchParams({ ...extra, page: String(page + 1) })}`;
}

type ListView = Pick<BlogListData, "kind" | "viewKey" | "title" | "subtitle" | "activeCategory" | "emptyText">;

async function scopedList(cms: BlogCms, url: URL, path: string, scope: { category?: string; author?: string }, view: ListView, categories: BlogCmsCategory[]): Promise<BlogOutcome<BlogListPayload>> {
  const page = parseBlogPage(url.searchParams.get("page"));
  const { posts, total } = await cms.list(scope, page);
  return {
    status: 200,
    payload: {
      state: "ok",
      ...view,
      query: "",
      posts: posts.map(toCard),
      total,
      nextHref: nextHref(path, page, total, listPageWindow),
      categories: menu(categories),
    },
  };
}

export function loadBlogHome(request: Request, env?: BaseEnv) {
  return guarded<BlogListPayload>(request, env, async (cms) =>
    scopedList(cms, new URL(request.url), BLOG_HOME, {}, {
      kind: "home",
      viewKey: "home",
      title: "The Decentraland Blog",
      subtitle: "Stories, guides and things to do in a world built by you.",
      activeCategory: "",
      emptyText: "There are no articles yet.",
    }, await cms.categories()),
  );
}

export function loadBlogCategory(request: Request, slug: string, env?: BaseEnv) {
  return guarded<BlogListPayload>(request, env, async (cms) => {
    const categories = await cms.categories();
    const category = categories.find((c) => c.slug === slug);
    if (!category) {
      // `/blog/<post-slug>` was a post's URL before posts moved under their category.
      const found = await cms.post(slug);
      return found?.categoryKnown ? { status: 301, location: blogPostHref(found.post.category.slug, found.post.slug) } : notFound("category");
    }
    return scopedList(cms, new URL(request.url), blogCategoryHref(slug), { category: slug }, {
      kind: "category",
      viewKey: `category:${slug}`,
      title: category.title,
      subtitle: category.description,
      activeCategory: slug,
      emptyText: "There are no articles in this category yet.",
    }, categories);
  });
}

export function loadBlogAuthor(request: Request, slug: string, env?: BaseEnv) {
  return guarded<BlogListPayload>(request, env, async (cms) => {
    const [authors, categories] = await Promise.all([cms.authors(), cms.categories()]);
    const author = authors.find((a) => a.slug === slug);
    if (!author) return notFound("author");
    return scopedList(cms, new URL(request.url), blogAuthorHref(slug), { author: slug }, {
      kind: "author",
      viewKey: `author:${slug}`,
      title: author.name,
      subtitle: author.description,
      activeCategory: null,
      emptyText: "This author has no articles yet.",
    }, categories);
  });
}

/** `/blog/search?q=`: cms-server's full-text search, as decentraland/sites; an empty `q` goes to the home. */
export function loadBlogSearch(request: Request, env?: BaseEnv) {
  const url = new URL(request.url);
  const q = parseBlogQuery(url.searchParams.get("q"));
  if (!q) return Promise.resolve({ status: 302, location: BLOG_HOME } as const);
  return guarded<BlogListPayload>(request, env, async (cms) => {
    const page = parseBlogPage(url.searchParams.get("page"));
    const short = q.length < SEARCH_MIN_LENGTH;
    const [categories, found] = await Promise.all([
      cms.categories(),
      short ? Promise.resolve({ posts: [] as BlogCmsPost[], total: 0 }) : cms.search(q, page),
    ]);
    return {
      status: 200,
      payload: {
        state: "ok",
        kind: "search",
        viewKey: null,
        title: "Search results",
        subtitle: "",
        activeCategory: null,
        query: q,
        posts: found.posts.map(toCard),
        total: found.total,
        nextHref: nextHref(BLOG_SEARCH, page, found.total, searchPageWindow, { q }, SEARCH_MAX_PAGES),
        emptyText: short
          ? `Type at least ${SEARCH_MIN_LENGTH} characters to search.`
          : `Nothing matches “${q}”. Try another word or browse every article.`,
        categories: menu(categories),
      },
    };
  });
}

/**
 * `/api/blog/search?q=`: the search dropdown's first matches, read through the same memoized search
 * as the results page, so typing and then opening "See more results" asks the CMS once. A query
 * under SEARCH_MIN_LENGTH asks nothing.
 */
export function loadBlogSearchSuggestions(request: Request, env?: BaseEnv): Promise<BlogOutcome<BlogSearchSuggestions>> {
  const query = parseBlogQuery(new URL(request.url).searchParams.get("q"));
  if (query.length < SEARCH_MIN_LENGTH) return Promise.resolve({ status: 200, payload: { query, posts: [], total: 0 } });
  return guarded<BlogSearchSuggestions>(request, env, async (cms) => {
    const found = await cms.search(query, 1);
    return {
      status: 200,
      payload: {
        query,
        total: found.total,
        posts: found.posts.slice(0, BLOG_SEARCH_SUGGESTIONS_LIMIT).map(({ slug, title, description, image, hue, category }) => ({
          slug,
          title,
          description,
          image,
          hue,
          href: blogPostHref(category.slug, slug),
        })),
      },
    };
  });
}

const RELATED = 3;

const related = (post: BlogCmsPost, pool: BlogCmsPost[]): BlogRelatedCard[] =>
  pool
    .filter((p, i, all) => p.slug !== post.slug && all.findIndex((x) => x.slug === p.slug) === i)
    .slice(0, RELATED)
    .map(({ slug, title, publishedAt, category, image, hue }) => ({ slug, title, publishedAt, category, image, hue }));

/**
 * Up to RELATED posts (the category's newest, then the blog's), within their own budget; null when
 * they fail or run past it, so a slow list never turns a loaded post into a 503.
 */
async function loadRelated(cms: BlogCms, post: BlogCmsPost, category: string | null, signal: AbortSignal): Promise<BlogRelatedCard[] | null> {
  try {
    const lists = await withDeadline(
      () => Promise.all(category ? [cms.list({ category }, 1), cms.list({}, 1)] : [cms.list({}, 1)]),
      BLOG_RELATED_TIMEOUT_MS,
      signal,
    );
    return related(post, lists.flatMap((list) => list.posts));
  } catch {
    signal.throwIfAborted();
    return null;
  }
}

export function loadBlogPost(request: Request, categorySlug: string, postSlug: string, env?: BaseEnv) {
  return guarded<BlogPostPayload>(request, env, async (cms, signal) => {
    const [found, categories] = await Promise.all([cms.post(postSlug), cms.categories()]);
    if (!found) return notFound("post");
    const { post, assets, categoryKnown } = found;
    // A category the CMS does not list (even after a reload) has no URL to send anyone to: the post
    // stays at the URL it was asked for. A permanent redirect is only ever to the post's own category.
    const href = blogPostHref(categoryKnown ? post.category.slug : categorySlug, post.slug);
    if (categoryKnown && post.category.slug !== categorySlug) return { status: 301, location: href };
    const relatedCards = await loadRelated(cms, post, categoryKnown ? post.category.slug : null, signal);
    const images: Record<string, BlogImage> = {};
    for (const [id, image] of assets) if (image) images[id] = image;
    return {
      status: 200,
      payload: {
        state: "ok",
        post: { ...toCard(post), body: post.body, assets: images },
        related: relatedCards,
        shareUrl: publicPageUrl(request, href, env),
        categories: menu(categories),
      },
    };
  });
}
