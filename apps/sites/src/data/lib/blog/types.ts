// Blog domain types, as the source Sites blog (decentraland/sites@cfdfa1a5,
// src/shared/blog/types/blog.domain.ts) shapes them from the CMS.

/** A Contentful rich-text node, as the CMS returns it (document, block, inline or text). */
export type RichTextNode = {
  nodeType: string;
  data?: Record<string, unknown>;
  content?: RichTextNode[];
  value?: string;
  marks?: { type: string }[];
};

export type BlogAsset = {
  id: string;
  url: string;
  width: number;
  height: number;
  mimeType: string;
  /** The asset's title, used as the image's alt text when present (blog plan R4-4). */
  title?: string;
};

export type BlogCategory = {
  id: string;
  slug: string;
  title: string;
  description: string;
  image: BlogAsset;
  isShownInMenu: boolean;
  url: string;
};

export type BlogAuthor = {
  id: string;
  slug: string;
  title: string;
  description: string;
  image: BlogAsset;
  url: string;
};

export type BlogPost = {
  id: string;
  slug: string;
  title: string;
  description: string;
  /** Raw ISO 8601 value from the CMS; render sites format it (en-US, UTC). */
  publishedDate: string;
  body: RichTextNode | null;
  /** Assets the body embeds or links, by id. */
  bodyAssets: Record<string, BlogAsset>;
  image: BlogAsset;
  category: BlogCategory;
  author: BlogAuthor;
  url: string;
  /**
   * Search results only: the title and description as highlight markup, rendered by the search
   * highlight (source cms.search.client.ts: the CMS's `_highlight` field, else the field as stored).
   */
  highlight?: BlogHighlight;
};

/** Highlight markup (em around the matches), never rendered as HTML. */
export type BlogHighlight = { title: string; description: string };

/** One page of a post list, as the list and search keys hold it inside InfiniteData. */
export type BlogPage = {
  items: BlogPost[];
  total: number;
  skip: number;
  limit: number;
};

export type BlogPageParam = { skip: number; limit: number };
