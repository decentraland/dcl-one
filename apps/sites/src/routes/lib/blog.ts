import { redirect, type HeadersArgs, type MetaDescriptor, type RouterContextProvider } from "react-router";
import { BLOG_HOME, blogPostHref } from "@dcl/one-ui";

import { parseBlogQuery, UNCATEGORIZED } from "@/data/lib/blog/normalize";
import type { BlogOutcome } from "@/data/lib/blog/pages";
import type { BlogListPayload, BlogPostPayload } from "@/data/lib/blog/types";
import { pageLoader, type PageDefaults } from "@/core/lib/telemetry/page-loader";

import { absoluteUrl, notFoundMeta, SITE_NAME, socialMeta, SCHEMA_ORG } from "./social-meta";

/** The blog keeps only https pictures, as decentraland.org's blog card did; anything else shows the default. */
const httpsImage = (url: string | null | undefined) => (url?.startsWith("https://") ? url : undefined);

const FALLBACK: PageDefaults = { variant: "index_grid" };

/**
 * A blog loader's response: the payload with its status (the view renders a 404 or 503 page from it),
 * or a redirect. A 503 is never cached; nothing about the upstream is in it.
 */
export async function blogResponse<T>(
  request: Request,
  context: Readonly<RouterContextProvider>,
  load: Promise<BlogOutcome<T>>,
) {
  const { sid, wrap } = await pageLoader(request, context, FALLBACK);
  const outcome = await load;
  if ("location" in outcome) throw redirect(outcome.location, outcome.status);
  const response = wrap({ sid, ...outcome.payload }, { status: outcome.status });
  if (outcome.status === 503) {
    const headers = new Headers(response.init?.headers);
    headers.set("Cache-Control", "no-store");
    headers.set("Retry-After", "300");
    response.init = { ...response.init, headers };
  }
  return response;
}

/** Document headers: the loader's Cache-Control and Retry-After (set on a 503) win over the parent's. */
export function blogHeaders({ loaderHeaders, parentHeaders }: HeadersArgs): Headers {
  const headers = new Headers(parentHeaders);
  for (const name of ["Cache-Control", "Retry-After"]) {
    const value = loaderHeaders.get(name);
    if (value) headers.set(name, value);
  }
  return headers;
}

export const BLOG_DEFAULT_TITLE = "Decentraland Blog";
export const BLOG_DEFAULT_DESCRIPTION = "Stay up to date with Decentraland announcements, updates, community highlights, and more.";
export const BLOG_DEFAULT_IMAGE =
  "https://cms-images.decentraland.org/ea2ybdmmn1kv/7tYISdowuJYIbSIDqij87H/f3524d454d8e29702792a6b674f5550d/GI_Landscape.Small.png";

type BlogCardInput = {
  /** The page's own title; the blog's default card (title without suffix) when omitted. */
  title?: string;
  description?: string;
  image?: string | null;
  url?: string;
  type?: "website" | "article";
  jsonLd?: readonly object[];
  extra?: readonly MetaDescriptor[];
};

/** A blog page's card, as decentraland.org's blog worker wrote it: `{title} | Decentraland` over the blog defaults. */
function blogCard({ title, description, image, ...rest }: BlogCardInput): MetaDescriptor[] {
  return socialMeta({
    title: title ? `${title} | ${SITE_NAME}` : BLOG_DEFAULT_TITLE,
    description: description || BLOG_DEFAULT_DESCRIPTION,
    image: httpsImage(image) ?? BLOG_DEFAULT_IMAGE,
    ...rest,
  });
}

/** The blog home: the newest post's description and picture, and the posts it lists as a schema.org Blog. */
export function blogIndexMeta(data: BlogListPayload | undefined, base: string | undefined, path: string): MetaDescriptor[] {
  const url = absoluteUrl(base, path);
  if (data?.state !== "ok") return blogCard({ url });
  const newest = data.posts[0];
  const blog = {
    "@context": SCHEMA_ORG,
    "@type": "Blog",
    name: BLOG_DEFAULT_TITLE,
    url: absoluteUrl(base, BLOG_HOME),
    blogPost: data.posts.map((post) => ({
      "@type": "BlogPosting",
      headline: post.title,
      datePublished: post.publishedAt || undefined,
      url: absoluteUrl(base, blogPostHref(post.category.slug, post.slug)),
    })),
  };
  return blogCard({ title: BLOG_DEFAULT_TITLE, description: newest?.description, image: newest?.image?.url, url, jsonLd: [blog] });
}

/** A category or author page; a missing one is a 404 without a card. */
export function blogListMeta(data: BlogListPayload | undefined): MetaDescriptor[] {
  if (data?.state === "not-found") return notFoundMeta();
  if (data?.state !== "ok") return blogCard({});
  return blogCard({ title: data.kind === "author" ? `Posts by ${data.title}` : data.title, description: data.subtitle });
}

/** `/blog/search?q=`: the query names the page; the CMS is not needed for the card. */
export function blogSearchMeta(search: string): MetaDescriptor[] {
  const q = parseBlogQuery(new URLSearchParams(search).get("q"));
  return blogCard({
    title: q ? `Search: ${q}` : "Search",
    description: q ? `Search results for "${q}" in ${BLOG_DEFAULT_TITLE}` : `Search ${BLOG_DEFAULT_TITLE}`,
  });
}

/**
 * A post: an article card when its author is known, with `article:*` tags once it also has a date, and
 * its schema.org BlogPosting. The canonical is the path without query or trailing slash.
 */
export function blogPostMeta(data: BlogPostPayload | undefined, base: string | undefined, pathname: string): MetaDescriptor[] {
  if (data?.state === "not-found") return notFoundMeta();
  const url = absoluteUrl(base, pathname.replace(/\/+$/, ""));
  if (data?.state !== "ok") return blogCard({ url });
  const { post } = data;
  const author = post.author.slug ? post.author.name : undefined;
  const section = post.category.slug !== UNCATEGORIZED.slug ? post.category.title : undefined;
  const published = post.publishedAt || undefined;
  const image = httpsImage(post.image?.url);
  const article = {
    "@context": SCHEMA_ORG,
    "@type": "BlogPosting",
    headline: post.title,
    url,
    mainEntityOfPage: url,
    publisher: { "@type": "Organization", name: SITE_NAME, url: url && new URL(url).origin },
    description: post.description || undefined,
    datePublished: published,
    author: author && { "@type": "Person", name: author },
    articleSection: section,
    image,
  };
  const extra: MetaDescriptor[] = [];
  if (author && published) {
    extra.push({ property: "article:author", content: author }, { property: "article:published_time", content: published });
    if (section) extra.push({ property: "article:section", content: section });
  }
  return blogCard({
    title: post.title,
    description: post.description,
    image,
    url,
    type: author ? "article" : "website",
    jsonLd: [article],
    extra,
  });
}
