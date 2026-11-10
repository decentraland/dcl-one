// from: sites@5aabce35: features/cms/cms.mappers.ts, cms.helpers.ts (getEntrySlug),
//       cms.search.client.ts (MIN_QUERY_LENGTH), useInfiniteBlogPosts.ts (page sizes),
//       shared/blog/utils/string.ts (slugify, decodeHtmlEntities)
// decentraland/sites is Apache-2.0.
// Turns raw cms-server entries into the blog's view values: the same entry yields the same slug,
// title and fallbacks as decentraland.org. Links (category, author, images) resolve from lookups the
// caller builds; nothing here fetches or caches.
import {
  BLOG_QUERY_MAX,
  type BlogAuthor,
  type BlogCard,
  type BlogCategory,
  type BlogImage,
  type BlogRichTextNode,
} from "@dcl/one-ui/apps/sites/blog/blogModel";

/** A raw CMS entry or asset: `sys.id` plus untyped, already localized `fields`. */
export type CmsEntry = { sys: { id: string }; fields?: Record<string, unknown> };

export type CmsList = { items: CmsEntry[]; total: number };

/** A category as the loaders use it: the view's slug and title, plus what the pages show. */
export type BlogCmsCategory = BlogCategory & { id: string; description: string; isShownInMenu: boolean };

/** An author with the CMS ids the loaders resolve: the entry's and its picture's. */
export type BlogCmsAuthor = { id: string; slug: string; name: string; description: string; imageId: string };

/** A post card plus what only the article page needs. */
export type BlogCmsPost = BlogCard & { id: string; body: BlogRichTextNode | null };

/** Assets the caller fetched, by id; null for one the CMS did not return. */
export type AssetLookup = ReadonlyMap<string, BlogImage | null>;

/** Home, category and author lists page as decentraland.org does: 7 posts, then 6 per "Load more". */
export const FIRST_PAGE_SIZE = 7;
export const NEXT_PAGE_SIZE = 6;
/** Search pages hold 10 posts; a query under 3 characters is not sent (as decentraland.org). */
export const SEARCH_PAGE_SIZE = 10;
export const SEARCH_MIN_LENGTH = 3;
/** The deepest `?page=` a list loads: each page is one CMS request, so the cap bounds a request's fan-out. */
export const MAX_PAGES = 20;
/** The deepest search page: a search reads its visible results in one request, and cms-server caps a page at 100. */
export const SEARCH_MAX_PAGES = 10;

const WORDS_PER_MINUTE = 200;

/** As sites: lower-case, drop non-word characters, collapse separators to single hyphens. */
export const slugify = (value: string): string =>
  value
    .toLowerCase()
    .trim()
    .replace(/[^\w\s-]/g, "")
    .replace(/[\s_-]+/g, "-")
    .replace(/^-+|-+$/g, "");

/** As sites: the named entities some entries were authored with; `&amp;` last so double-encoded text stays encoded once. */
export const decodeHtmlEntities = (value: string): string =>
  value
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;|&apos;/g, "'")
    .replace(/&nbsp;/g, " ")
    .replace(/&amp;/g, "&");

const text = (value: unknown): string => (typeof value === "string" ? value : "");
const decoded = (value: unknown): string => decodeHtmlEntities(text(value));

/** The id a `{ sys: { id } }` link points at, or "". */
export const linkId = (value: unknown): string => {
  const id = (value as { sys?: { id?: unknown } } | null | undefined)?.sys?.id;
  return typeof id === "string" ? id : "";
};

/** `?q=` trimmed and capped; "" means no search. */
export const parseBlogQuery = (value: string | null | undefined): string =>
  (value ?? "").trim().slice(0, BLOG_QUERY_MAX).trim();

/** `?page=`: a whole number from 1 to MAX_PAGES; anything else is the first page. */
export function parseBlogPage(value: string | null | undefined): number {
  const n = Number(value);
  return Number.isInteger(n) && n >= 1 ? Math.min(n, MAX_PAGES) : 1;
}

type FileField = { url?: unknown; details?: { image?: { width?: unknown; height?: unknown } } };

/** An asset as an image; null without a file URL. Protocol-relative URLs get https, as sites. */
export function mapAsset(asset: CmsEntry | null | undefined): BlogImage | null {
  const file = asset?.fields?.file as FileField | undefined;
  const url = text(file?.url);
  if (!url) return null;
  const size = (v: unknown) => (typeof v === "number" && v > 0 ? v : 0);
  return {
    url: url.startsWith("//") ? `https:${url}` : url,
    width: size(file?.details?.image?.width),
    height: size(file?.details?.image?.height),
    alt: decoded(asset?.fields?.title).trim(),
  };
}

/** As sites: the slug is `fields.id`, else the raw title slugified (decoding never moves a URL). */
export function mapCategory(entry: CmsEntry): BlogCmsCategory | null {
  const fields = entry.fields ?? {};
  const slug = text(fields.id) || slugify(text(fields.title));
  if (!entry.sys?.id || !slug) return null;
  return {
    id: entry.sys.id,
    slug,
    title: decoded(fields.title) || slug,
    description: decoded(fields.description),
    isShownInMenu: fields.isShownInMenu !== false,
  };
}

/** As sites: `fields.id`, else the slugified title, else the entry id. */
export function mapAuthor(entry: CmsEntry): BlogCmsAuthor | null {
  const fields = entry.fields ?? {};
  if (!entry.sys?.id) return null;
  return {
    id: entry.sys.id,
    slug: text(fields.id) || slugify(text(fields.title)) || entry.sys.id,
    name: decoded(fields.title) || "Decentraland",
    description: decoded(fields.description),
    imageId: linkId(fields.image),
  };
}

/** A post without a known category, as sites. */
const UNCATEGORIZED: BlogCategory = { slug: "uncategorized", title: "Uncategorized" };
/** A post without a known author, as sites names it; no page to link. */
const NO_AUTHOR: BlogAuthor = { slug: "", name: "Decentraland", image: "" };

/** The UTC calendar date of a CMS date-time (sites formats it in UTC), or "". */
export function publishedAt(value: unknown): string {
  const raw = text(value);
  if (!raw) return "";
  const date = new Date(raw.includes("T") ? raw : `${raw}T00:00:00Z`);
  return Number.isNaN(date.getTime()) ? "" : date.toISOString().slice(0, 10);
}

/** The words of a rich-text body, for reading time and plain-text uses. */
export function richTextWords(body: BlogRichTextNode | null | undefined): number {
  let words = 0;
  const walk = (node: BlogRichTextNode) => {
    if (node.nodeType === "text" && typeof node.value === "string") words += node.value.split(/\s+/).filter(Boolean).length;
    if (Array.isArray(node.content)) node.content.forEach(walk);
  };
  if (body) walk(body);
  return words;
}

export const readMinutes = (body: BlogRichTextNode | null | undefined): number =>
  Math.max(1, Math.ceil(richTextWords(body) / WORDS_PER_MINUTE));

/** Ids of the assets a body embeds. */
export function bodyAssetIds(body: BlogRichTextNode | null | undefined): string[] {
  const ids = new Set<string>();
  const walk = (node: BlogRichTextNode) => {
    if (node.nodeType === "embedded-asset-block") {
      const id = linkId(node.data?.target);
      if (id) ids.add(id);
    }
    if (Array.isArray(node.content)) node.content.forEach(walk);
  };
  if (body) walk(body);
  return [...ids];
}

/** A stable hue (0-359) from the slug, for the gradient shown when a post has no picture. */
export function blogHue(slug: string): number {
  let hash = 0;
  for (const ch of slug) hash = (hash * 31 + ch.charCodeAt(0)) % 360;
  return hash;
}

export type PostLookups = {
  categories: ReadonlyMap<string, BlogCmsCategory>;
  authors: ReadonlyMap<string, BlogCmsAuthor>;
  assets: AssetLookup;
};

/** The asset ids a list of post entries needs: each post's picture and its author's. */
export function postAssetIds(entries: CmsEntry[], authors: ReadonlyMap<string, BlogCmsAuthor>): string[] {
  const ids = new Set<string>();
  for (const entry of entries) {
    const image = linkId(entry.fields?.image);
    if (image) ids.add(image);
    const author = authors.get(linkId(entry.fields?.author));
    if (author?.imageId) ids.add(author.imageId);
  }
  return [...ids];
}

/** A post entry as a card and body; null without a slug or a title. */
export function mapPost(entry: CmsEntry, lookups: PostLookups): BlogCmsPost | null {
  const fields = entry.fields ?? {};
  const slug = text(fields.id);
  const title = decoded(fields.title);
  if (!entry.sys?.id || !slug || !title) return null;
  const category = lookups.categories.get(linkId(fields.category));
  const author = lookups.authors.get(linkId(fields.author));
  const body =
    fields.body && typeof fields.body === "object" ? (fields.body as BlogRichTextNode) : null;
  return {
    id: entry.sys.id,
    slug,
    title,
    description: decoded(fields.description),
    publishedAt: publishedAt(fields.publishedDate),
    readMinutes: readMinutes(body),
    category: category ? { slug: category.slug, title: category.title } : UNCATEGORIZED,
    author: author
      ? { slug: author.slug, name: author.name, image: lookups.assets.get(author.imageId)?.url ?? "" }
      : NO_AUTHOR,
    image: lookups.assets.get(linkId(fields.image)) ?? null,
    hue: blogHue(slug),
    body,
  };
}

/** A card without the body, for lists. */
export function toCard(post: BlogCmsPost): BlogCard {
  const { id: _id, body: _body, ...card } = post;
  void _id;
  void _body;
  return card;
}

/** Reads a list response; null when it is not `{ items: [], total: number }`. */
export function asCmsList(value: unknown): CmsList | null {
  const v = value as { items?: unknown; total?: unknown } | null;
  if (!v || typeof v !== "object" || !Array.isArray(v.items)) return null;
  const items = v.items.filter(
    (item): item is CmsEntry =>
      !!item && typeof item === "object" && typeof (item as CmsEntry).sys?.id === "string",
  );
  return { items, total: typeof v.total === "number" && v.total >= 0 ? v.total : items.length };
}
