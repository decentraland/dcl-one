// Turns raw CMS entries into blog domain values, porting the source Sites mappers
// (decentraland/sites@cfdfa1a5: cms.mappers.ts in the cms feature, and the shared blog utils) so the
// same entry yields the same title, slug, URL and fallback. References (category, author, images)
// are resolved from a per-request lookup the caller builds; nothing here is cached.
import type {
  BlogAsset,
  BlogAuthor,
  BlogCategory,
  BlogPage,
  BlogPageParam,
  BlogPost,
  RichTextNode,
} from "./types";

/** A raw CMS entry or asset: `sys.id` plus untyped `fields`. */
export type CmsEntry = {
  sys: { id: string; type?: string };
  fields?: Record<string, unknown>;
};
type CmsLink = {
  sys: { id: string; type: "Link"; linkType: "Entry" | "Asset" };
};

/** Entries and assets the caller has fetched, by id, for resolving links. */
export type CmsLookup = {
  entries: ReadonlyMap<string, CmsEntry>;
  assets: ReadonlyMap<string, CmsEntry>;
};

const FALLBACK_LOGO = "https://decentraland.org/logos/png/color.png";

export const blogPaths = {
  home: () => "/blog",
  category: (categorySlug: string) => `/blog/${categorySlug}`,
  post: (categorySlug: string, postSlug: string) =>
    `/blog/${categorySlug}/${postSlug}`,
  author: (authorSlug: string) => `/blog/author/${authorSlug}`,
  search: (query: string) => `/blog/search?q=${encodeURIComponent(query)}`,
};

/** As source: lower-case, drop non-word characters, collapse separators to single hyphens. */
export const slugify = (value: string | null | undefined): string =>
  value
    ? value
        .toLowerCase()
        .trim()
        .replace(/[^\w\s-]/g, "")
        .replace(/[\s_-]+/g, "-")
        .replace(/^-+|-+$/g, "")
    : "";

/**
 * As source: decodes the named entities some CMS entries were authored with. `&nbsp;` becomes a
 * plain space and `&amp;` is decoded last, so double-encoded markup stays encoded once; numeric
 * references are left as they are.
 */
export const decodeHtmlEntities = (value: string): string =>
  value
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;|&apos;/g, "'")
    .replace(/&nbsp;/g, " ")
    .replace(/&amp;/g, "&");

const decodeField = (value: unknown) =>
  typeof value === "string" ? decodeHtmlEntities(value) : "";

const isLink = (
  value: unknown,
  linkType: "Entry" | "Asset",
): value is CmsLink => {
  const sys = (value as CmsLink | undefined)?.sys;
  return sys?.type === "Link" && sys.linkType === linkType;
};

const resolveAsset = (value: unknown, lookup: CmsLookup): CmsEntry | null =>
  isLink(value, "Asset")
    ? (lookup.assets.get(value.sys.id) ?? null)
    : ((value as CmsEntry | null) ?? null);

const resolveEntry = (value: unknown, lookup: CmsLookup): CmsEntry | null =>
  isLink(value, "Entry")
    ? (lookup.entries.get(value.sys.id) ?? null)
    : ((value as CmsEntry | null) ?? null);

type FileField = {
  url?: string;
  contentType?: string;
  details?: { image?: { width?: number; height?: number } };
};

export function mapAsset(asset: CmsEntry | null): BlogAsset | null {
  const file = asset?.fields?.file as FileField | undefined;
  if (!asset?.sys || !file?.url) return null;
  const title =
    typeof asset.fields?.title === "string" && asset.fields.title.trim()
      ? asset.fields.title
      : undefined;
  return {
    id: asset.sys.id,
    url: file.url.startsWith("//") ? `https:${file.url}` : file.url,
    width: file.details?.image?.width || 0,
    height: file.details?.image?.height || 0,
    mimeType: file.contentType || "image/jpeg",
    ...(title ? { title } : {}),
  };
}

const fallbackImage = (
  id: string,
  width: number,
  height: number,
): BlogAsset => ({
  id,
  url: FALLBACK_LOGO,
  width,
  height,
  mimeType: "image/png",
});

export function mapCategory(
  entry: CmsEntry | null,
  lookup: CmsLookup,
): BlogCategory | null {
  if (!entry?.sys || !entry.fields) return null;
  // As source: the slug derives from the raw title, so decoding never changes an existing URL.
  const rawTitle = (entry.fields.title as string | undefined) || "";
  const slug = (entry.fields.id as string | undefined) || slugify(rawTitle);
  if (!slug) return null;
  const image = mapAsset(resolveAsset(entry.fields.image, lookup));
  if (!image) return null;
  return {
    id: entry.sys.id,
    slug,
    title: decodeHtmlEntities(rawTitle),
    description: decodeField(entry.fields.description),
    image,
    isShownInMenu: (entry.fields.isShownInMenu as boolean | undefined) ?? true,
    url: blogPaths.category(slug),
  };
}

const defaultCategory = (postId: string): BlogCategory => ({
  id: postId || "uncategorized",
  slug: "uncategorized",
  title: "Uncategorized",
  description: "Posts without a valid category",
  image: fallbackImage("default-category-image", 200, 200),
  isShownInMenu: false,
  url: blogPaths.category("uncategorized"),
});

export function mapAuthor(
  entry: CmsEntry | null,
  lookup: CmsLookup,
): BlogAuthor {
  if (!entry?.sys || !entry.fields || Object.keys(entry.fields).length === 0) {
    const slug = entry?.sys?.id || "decentraland";
    return {
      id: entry?.sys?.id || "unknown",
      slug,
      title: "Decentraland",
      description: "Decentraland Team",
      image: fallbackImage("default-avatar", 200, 200),
      url: blogPaths.author(slug),
    };
  }
  const rawTitle = (entry.fields.title as string | undefined) || "";
  const title = decodeHtmlEntities(rawTitle);
  const slug =
    (entry.fields.id as string | undefined) ||
    slugify(rawTitle) ||
    entry.sys.id;
  const image = mapAsset(resolveAsset(entry.fields.image, lookup));
  return {
    id: entry.sys.id,
    slug,
    title: image ? title : title || "Decentraland",
    description: decodeField(entry.fields.description),
    image: image ?? fallbackImage("default-avatar", 200, 200),
    url: blogPaths.author(slug),
  };
}

/** Ids of every asset the body embeds (embedded-asset-block/inline) or links (asset-hyperlink). */
export function bodyAssetIds(body: RichTextNode | null | undefined): string[] {
  const ids = new Set<string>();
  const walk = (node: RichTextNode) => {
    if (
      /^(embedded-asset-(block|inline)|asset-hyperlink)$/.test(node.nodeType)
    ) {
      const id = (node.data?.target as CmsLink | undefined)?.sys?.id;
      if (id) ids.add(id);
    }
    node.content?.forEach(walk);
  };
  if (body) walk(body);
  return [...ids];
}

export function mapPost(
  entry: CmsEntry | null,
  lookup: CmsLookup,
): BlogPost | null {
  if (!entry?.sys || !entry.fields) return null;
  const category =
    mapCategory(resolveEntry(entry.fields.category, lookup), lookup) ??
    defaultCategory(entry.sys.id);
  const author = mapAuthor(resolveEntry(entry.fields.author, lookup), lookup);
  const image =
    mapAsset(resolveAsset(entry.fields.image, lookup)) ??
    fallbackImage(entry.sys.id || "default-image", 1200, 630);
  const body = (entry.fields.body as RichTextNode | undefined) ?? null;
  const bodyAssets: Record<string, BlogAsset> = {};
  for (const id of bodyAssetIds(body)) {
    const asset = mapAsset(lookup.assets.get(id) ?? null);
    if (asset) bodyAssets[id] = asset;
  }
  const slug = entry.fields.id as string;
  return {
    id: entry.sys.id,
    slug,
    title: decodeField(entry.fields.title),
    description: decodeField(entry.fields.description),
    publishedDate: (entry.fields.publishedDate as string | undefined) ?? "",
    body,
    bodyAssets,
    image,
    category,
    author,
    url: blogPaths.post(category.slug, slug),
  };
}

/** A /blog/posts response as one list page; unmappable entries are dropped, as source. */
export function mapPage(
  response: { items?: CmsEntry[]; total?: number },
  param: BlogPageParam,
  lookup: CmsLookup,
): BlogPage {
  return {
    items: (response.items ?? [])
      .map((item) => mapPost(item, lookup))
      .filter((post): post is BlogPost => post !== null),
    total: response.total ?? 0,
    skip: param.skip,
    limit: param.limit,
  };
}

const rawField = (value: unknown) => (typeof value === "string" ? value : "");

/**
 * A search response as one page: each post carries its highlight markup, the CMS's `_highlight`
 * title and description when present, else the stored field undecoded (source cms.search.client.ts,
 * which hands either to its sanitizer as HTML).
 */
export function mapSearchPage(
  response: {
    items?: (CmsEntry & {
      _highlight?: { title?: unknown; description?: unknown };
    })[];
    total?: number;
  },
  param: BlogPageParam,
  lookup: CmsLookup,
): BlogPage {
  const items: BlogPost[] = [];
  for (const item of response.items ?? []) {
    const post = mapPost(item, lookup);
    if (!post) continue;
    const marked = item._highlight ?? {};
    post.highlight = {
      title:
        typeof marked.title === "string"
          ? marked.title
          : rawField(item.fields?.title),
      description:
        typeof marked.description === "string"
          ? marked.description
          : rawField(item.fields?.description),
    };
    items.push(post);
  }
  return {
    items,
    total: response.total ?? 0,
    skip: param.skip,
    limit: param.limit,
  };
}

/** Source pages the home, category and author lists in 7 then 6 (useInfiniteBlogPosts.ts). */
export const FIRST_PAGE: BlogPageParam = { skip: 0, limit: 7 };
export const NEXT_PAGE_LIMIT = 6;

/**
 * The next page to request, or undefined at the end. As source (cms.client.ts:186-187): the next skip
 * is this page's skip plus the items it returned, and an empty page ends the list.
 */
export function nextPageParam(page: BlogPage): BlogPageParam | undefined {
  const next = page.skip + page.items.length;
  return page.items.length > 0 && next < page.total
    ? { skip: next, limit: NEXT_PAGE_LIMIT }
    : undefined;
}
