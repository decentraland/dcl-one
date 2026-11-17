// The blog's reads from cms-server (BLOG_CMS_URL): GET /blog/{posts,categories,authors} and
// GET /assets/{id}, all public and anonymous. Every failure (unset base, network, timeout, non-2xx,
// a body that is not the expected JSON) is a BlogUnavailableError, which the routes answer with a
// generic 503; nothing about the upstream reaches the response.
import type { BlogImage } from "@dcl/one-ui/apps/sites/blog/blogModel";

import { blogCmsUrl, type BaseEnv } from "../catalyst/bases";
import { ttlMemo } from "../ttl-memo";
import {
  asCmsList,
  bodyAssetIds,
  FIRST_PAGE_SIZE,
  linkId,
  mapAsset,
  mapAuthor,
  mapCategory,
  mapPost,
  NEXT_PAGE_SIZE,
  postAssetIds,
  SEARCH_MAX_PAGES,
  SEARCH_PAGE_SIZE,
  type BlogCmsAuthor,
  type BlogCmsCategory,
  type BlogCmsPost,
  type CmsEntry,
  type CmsList,
} from "./normalize";

export class BlogUnavailableError extends Error {
  constructor(options?: { cause?: unknown }) {
    super("The blog is temporarily unavailable.", options);
    this.name = "BlogUnavailableError";
  }
}

/** One CMS request; shared (memoized) reads outlive the visitor's request, so they carry their own deadline. */
export const CMS_REQUEST_TIMEOUT_MS = 6_000;
// cms-server serves en-US, es and zh; the blog is English only, as decentraland/sites'.
const LOCALE = "en-US";
const MEMO_TTL_MS = 5 * 60_000;
/** How long an expired list page is still served while one reload runs behind it (/create's 1s budget). */
const LIST_STALE_MS = 60 * 60_000;
/** Slugs that miss (no such post) and search results: short-lived and bounded, they cap anonymous load. */
const MISS_TTL_MS = 60_000;
const MISS_MAX = 500;
const SEARCH_TTL_MS = 60_000;
const SEARCH_MAX = 200;
/** At most one forced category reload per base in this window, when a post names an unknown category. */
const CATEGORY_REFRESH_MS = 60_000;

/**
 * Category slugs the static routes `/blog/search` and `/blog/author/*` shadow: a CMS category with
 * one of these slugs could never be reached, so it is left out of the categories (and logged once).
 */
export const RESERVED_CATEGORY_SLUGS: ReadonlySet<string> = new Set(["search", "author"]);
const reservedLogged = new Set<string>();

function withoutReserved(categories: BlogCmsCategory[]): BlogCmsCategory[] {
  return categories.filter((c) => {
    if (!RESERVED_CATEGORY_SLUGS.has(c.slug)) return true;
    if (!reservedLogged.has(c.slug)) {
      reservedLogged.add(c.slug);
      console.warn(`[blog] CMS category "${c.slug}" is shadowed by a static /blog route and is not shown`);
    }
    return false;
  });
}

/** The search memo's key: the query as the CMS matches it (case and spacing do not change results). */
export const normalizeSearchQuery = (q: string): string => q.trim().replace(/\s+/g, " ").toLowerCase();

type Params = Record<string, string | number | undefined>;

/** The request URL: the base plus a path and the params that are set. */
export function cmsUrl(base: string, path: string, params: Params = {}): string {
  const url = new URL(`${base}${path}`);
  for (const [key, value] of Object.entries(params)) if (value !== undefined && value !== "") url.searchParams.set(key, String(value));
  return url.toString();
}

/** GETs JSON; `allowMissing` turns a 404 into null (a single asset). */
async function cmsGet(url: string, signal: AbortSignal, allowMissing = false): Promise<unknown> {
  let response: Response;
  try {
    response = await fetch(url, { signal, headers: { accept: "application/json" } });
  } catch (cause) {
    throw new BlogUnavailableError({ cause });
  }
  if (!response.ok) {
    // An unread body keeps its connection busy; during an outage every 404 or 5xx would hold one.
    await response.body?.cancel().catch(() => {});
    if (allowMissing && response.status === 404) return null;
    throw new BlogUnavailableError({ cause: response.status });
  }
  try {
    return await response.json();
  } catch (cause) {
    throw new BlogUnavailableError({ cause });
  }
}

async function cmsList(url: string, signal: AbortSignal): Promise<CmsList> {
  const list = asCmsList(await cmsGet(url, signal));
  if (!list) throw new BlogUnavailableError({ cause: "unexpected list shape" });
  return list;
}

const ownSignal = () => AbortSignal.timeout(CMS_REQUEST_TIMEOUT_MS);

// Visitor-independent reads, memoized per process. Keyed by the base URL and every param, so a base
// change never serves another CMS's data; BLOG_CMS_URL is read from the process environment, constant
// for the process. Keys are bounded: list scopes are checked against the CMS's own categories and
// authors before a page is read, and pages are capped (MAX_PAGES).
const categoriesMemo = ttlMemo({
  ttlMs: MEMO_TTL_MS,
  keyOf: (base: string) => base,
  load: async (base: string) =>
    withoutReserved(
      (await cmsList(cmsUrl(base, "/blog/categories", { locale: LOCALE, limit: 100 }), ownSignal())).items
        .map(mapCategory)
        .filter((c): c is BlogCmsCategory => c !== null),
    ),
});

const authorsMemo = ttlMemo({
  ttlMs: MEMO_TTL_MS,
  keyOf: (base: string) => base,
  load: async (base: string) =>
    (await cmsList(cmsUrl(base, "/blog/authors", { locale: LOCALE, limit: 100 }), ownSignal())).items
      .map(mapAuthor)
      .filter((a): a is BlogCmsAuthor => a !== null),
});

// A missing or unreadable asset is not retained, so the next request tries again.
const assetMemo = ttlMemo({
  ttlMs: MEMO_TTL_MS,
  keyOf: (base: string, id: string) => `${base}\n${id}`,
  keep: (image: BlogImage | null) => image !== null,
  load: async (base: string, id: string) =>
    mapAsset((await cmsGet(cmsUrl(base, `/assets/${encodeURIComponent(id)}`, { locale: LOCALE }), ownSignal(), true)) as CmsEntry | null),
});

/** A list scope: every post, a category's or an author's (by slug). */
export type BlogScope = { category?: string; author?: string };

export type BlogPage = { posts: BlogCmsPost[]; total: number };

/** Page `n` (1-based) of a list: 7 posts first, then 6 each, as decentraland/sites. */
export function listPageWindow(n: number): { skip: number; limit: number } {
  return n === 1 ? { skip: 0, limit: FIRST_PAGE_SIZE } : { skip: FIRST_PAGE_SIZE + (n - 2) * NEXT_PAGE_SIZE, limit: NEXT_PAGE_SIZE };
}

export function searchPageWindow(n: number): { skip: number; limit: number } {
  return { skip: (n - 1) * SEARCH_PAGE_SIZE, limit: SEARCH_PAGE_SIZE };
}

/** The blog's CMS reads for one base URL. */
export type BlogCms = ReturnType<typeof createBlogCms>;

export function createBlogCms(base: string) {
  let categoriesRefreshedAt = -Infinity;
  const lookups = async () => {
    const [categories, authors] = await Promise.all([categoriesMemo(base), authorsMemo(base)]);
    return {
      categories: new Map(categories.map((c) => [c.id, c])),
      authors: new Map(authors.map((a) => [a.id, a])),
    };
  };

  const assets = async (ids: string[]) => {
    // A picture that fails to load falls back to the gradient; it never fails the page.
    const images = await Promise.all(ids.map((id) => assetMemo(base, id).catch(() => null)));
    return new Map(ids.map((id, i) => [id, images[i] ?? null]));
  };

  /** Maps a list response, resolving the pictures it needs plus `extraAssets` in one batch. */
  const mapPage = async (list: CmsList, extraAssets: string[] = []) => {
    const { categories, authors } = await lookups();
    const resolved = await assets([...new Set([...postAssetIds(list.items, authors), ...extraAssets])]);
    const posts = list.items
      .map((entry) => mapPost(entry, { categories, authors, assets: resolved }))
      .filter((p): p is BlogCmsPost => p !== null);
    return { posts, total: list.total, assets: resolved, categories };
  };

  const listPage = ttlMemo({
    ttlMs: MEMO_TTL_MS,
    staleMs: LIST_STALE_MS,
    keyOf: (scope: BlogScope, page: number) => `${base}\n${scope.category ?? ""}\n${scope.author ?? ""}\n${page}`,
    load: async (scope: BlogScope, page: number): Promise<BlogPage> => {
      const { skip, limit } = listPageWindow(page);
      const list = await cmsList(
        cmsUrl(base, "/blog/posts", { locale: LOCALE, category: scope.category, author: scope.author, limit, skip }),
        ownSignal(),
      );
      const mapped = await mapPage(list);
      // Lists keep cards only: the bodies are what makes a page of posts heavy.
      return { total: mapped.total, posts: mapped.posts.map((p) => ({ ...p, body: null })) };
    },
  });

  const searchMemo = ttlMemo({
    ttlMs: SEARCH_TTL_MS,
    maxEntries: SEARCH_MAX,
    keyOf: (q: string, pages: number) => `${q}\n${pages}`,
    load: async (q: string, pages: number): Promise<BlogPage> => {
      const last = searchPageWindow(pages);
      const page = await mapPage(
        await cmsList(cmsUrl(base, "/blog/posts", { q, locale: LOCALE, limit: last.skip + last.limit, skip: 0 }), ownSignal()),
      );
      return { posts: uniqueBySlug(page.posts).map((p) => ({ ...p, body: null })), total: page.total };
    },
  });

  // Only misses are kept: an unknown slug (an old link, a typo, a scanner) asks the CMS once a minute.
  const postEntry = ttlMemo({
    ttlMs: MISS_TTL_MS,
    maxEntries: MISS_MAX,
    keyOf: (slug: string) => slug,
    keep: (entry: CmsEntry | null) => entry === null,
    load: async (slug: string): Promise<CmsEntry | null> => {
      const list = await cmsList(cmsUrl(base, "/blog/posts", { locale: LOCALE, slug, limit: 1 }), ownSignal());
      return list.items.find((item) => item.fields?.id === slug) ?? null;
    },
  });

  return {
    categories: () => categoriesMemo(base),
    authors: () => authorsMemo(base),

    /** Pages 1..`pages` of a list, in order, without repeats (a post can shift between cached pages). */
    async list(scope: BlogScope, pages: number): Promise<{ posts: BlogCmsPost[]; total: number }> {
      const loaded = await Promise.all(Array.from({ length: pages }, (_, i) => listPage(scope, i + 1)));
      return { posts: uniqueBySlug(loaded.flatMap((p) => p.posts)), total: loaded[0]?.total ?? 0 };
    },

    /**
     * Pages 1..`pages` of the full-text search (cms-server's `q`), best match first: one upstream
     * request whatever the page (the visible results in a single read, capped at cms-server's
     * 100-item limit, SEARCH_MAX_PAGES), kept briefly in a bounded memo keyed by the normalized query.
     */
    search(q: string, pages: number): Promise<BlogPage> {
      return searchMemo(normalizeSearchQuery(q), Math.min(Math.max(pages, 1), SEARCH_MAX_PAGES));
    },

    /**
     * A post by its slug with the pictures its body embeds, or null when there is none.
     * `categoryKnown` is false when the post's category is not among the CMS's categories even after
     * one reload: its `category` is then a placeholder, not a URL to send anyone to.
     */
    async post(slug: string): Promise<{ post: BlogCmsPost; assets: Map<string, BlogImage | null>; categoryKnown: boolean } | null> {
      const entry = await postEntry(slug);
      if (!entry) return null;
      const categoryId = linkId(entry.fields?.category);
      // Categories are memoized: a category created after the last read is not in them yet.
      if (categoryId && Date.now() - categoriesRefreshedAt >= CATEGORY_REFRESH_MS) {
        const known = (await categoriesMemo(base)).some((c) => c.id === categoryId);
        if (!known) {
          categoriesRefreshedAt = Date.now();
          categoriesMemo.forget(base);
        }
      }
      const embedded = bodyAssetIds((entry.fields?.body ?? null) as BlogCmsPost["body"]);
      const page = await mapPage({ items: [entry], total: 1 }, embedded);
      const post = page.posts[0];
      if (!post) return null;
      return {
        post,
        assets: new Map(embedded.map((id) => [id, page.assets.get(id) ?? null])),
        categoryKnown: page.categories.has(categoryId),
      };
    },
  };
}

function uniqueBySlug(posts: BlogCmsPost[]): BlogCmsPost[] {
  const seen = new Set<string>();
  return posts.filter((p) => !seen.has(p.slug) && (seen.add(p.slug), true));
}

/** The CMS for this process's BLOG_CMS_URL; throws BlogUnavailableError when it is unset. */
export function blogCms(env?: BaseEnv): BlogCms {
  const base = env ? blogCmsUrl(env) : blogCmsUrl();
  if (!base) throw new BlogUnavailableError({ cause: "BLOG_CMS_URL is not set" });
  return cmsFor(base);
}

const instances = new Map<string, BlogCms>();
function cmsFor(base: string): BlogCms {
  let cms = instances.get(base);
  if (!cms) {
    cms = createBlogCms(base);
    instances.set(base, cms);
  }
  return cms;
}

/** Clears every memo (tests). */
export function resetBlogCms() {
  categoriesMemo.reset();
  authorsMemo.reset();
  assetMemo.reset();
  instances.clear();
  reservedLogged.clear();
}
