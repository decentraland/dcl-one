// What every blog route shares (blog plan B1, R4, R7): the chrome's locale messages, the source's
// unavailable/not-found answers, and page meta. Blog clientLoaders never call serverLoader (R7): on a
// client navigation they load messages in the browser and read data through the page's QueryClient.
import { siteUrl } from "@/data/lib/catalyst/bases";
import {
  BlogSourceUnavailable,
  blogUnavailableResponse,
} from "@/data/lib/blog/source";
import { loadLocaleMessages, type LocaleMessages } from "../locale/catalogs";
import { resolveRequestLocale } from "../locale/locales";

/** Server: the chrome's messages, resolved as /download resolves them. */
export const serverMessages = (request: Request): Promise<LocaleMessages> =>
  loadLocaleMessages(resolveRequestLocale(request));

/** Runs a source read; an unavailable source becomes the generic 503 (thrown to the route). */
export async function readBlog<T>(read: () => Promise<T>): Promise<T> {
  try {
    return await read();
  } catch (error) {
    if (error instanceof BlogSourceUnavailable) throw blogUnavailableResponse();
    throw error;
  }
}

/**
 * Production's answer for a missing post, category or author (blog plan R4-2): 404, not cached, not
 * indexed. The body carries the page's chrome data, so the error page keeps the header and sub-nav and
 * shows the source's error copy.
 */
export function blogNotFound(kind: BlogMissing, base: unknown): never {
  throw new Response(JSON.stringify({ kind, base }), {
    status: 404,
    headers: {
      "Content-Type": "application/json",
      "Cache-Control": "no-store",
      "X-Robots-Tag": "noindex",
    },
  });
}

export type BlogMissing = "post" | "category" | "author";

/** The source's English error copy (src/intl/en.json, error.load_*). */
export const BLOG_NOT_FOUND_COPY: Record<BlogMissing, string> = {
  post: "Failed to load post. Please try again later.",
  category: "Failed to load category. Please try again later.",
  author: "Failed to load author. Please try again later.",
};

/** "{title} | Decentraland", as the source's SEO component writes page titles. */
export const blogTitle = (title: string) => `${title} | Decentraland`;

/**
 * The deployment's public origin from configuration (PUBLIC_CATALYST_URL through siteUrl, the same on
 * the server and in the browser), or "" when none is set. Never derived from the request: a forged
 * Host or X-Forwarded-Host must not change canonical URLs or which links render as internal.
 */
export const blogPublicBase = (): string => siteUrl("/").replace(/\/$/, "");

/** The page's URL for canonical and og:url: absolute under the configured origin, else the path. */
export const blogCanonical = (publicBase: string, path: string) =>
  `${publicBase}${path}`;

/** The source's SEO component (components/blog/SEO/SEO.tsx), as route meta. */
export type BlogSeo = {
  title?: string;
  description: string;
  canonical: string;
  type?: "website" | "article" | "profile";
  image?: { url: string; width?: number; height?: number; alt?: string };
  article?: {
    author?: string;
    publishedTime?: string;
    section?: string;
    tags?: string[];
  };
};

export const BLOG_DEFAULT_DESCRIPTION =
  "Stay up to date with Decentraland announcements, updates, community highlights, and more.";

const DEFAULT_IMAGE = {
  url: "https://cms-images.decentraland.org/ea2ybdmmn1kv/7tYISdowuJYIbSIDqij87H/f3524d454d8e29702792a6b674f5550d/GI_Landscape.Small.png",
  width: 1200,
  height: 630,
  alt: "Decentraland - A virtual world owned by its users",
  type: "image/jpeg",
};

export function blogMeta(seo: BlogSeo) {
  const title = seo.title ? blogTitle(seo.title) : "Decentraland";
  const image: {
    url: string;
    width?: number;
    height?: number;
    alt?: string;
    type?: string;
  } = seo.image ?? DEFAULT_IMAGE;
  const type = seo.type ?? "website";
  const article = type === "article" ? seo.article : undefined;
  return [
    { title },
    { name: "description", content: seo.description },
    { tagName: "link", rel: "canonical", href: seo.canonical },
    { property: "og:site_name", content: "Decentraland" },
    { property: "og:title", content: title },
    { property: "og:description", content: seo.description },
    { property: "og:type", content: type },
    { property: "og:url", content: seo.canonical },
    { property: "og:locale", content: "en_US" },
    { property: "og:image", content: image.url },
    ...(image.width
      ? [{ property: "og:image:width", content: String(image.width) }]
      : []),
    ...(image.height
      ? [{ property: "og:image:height", content: String(image.height) }]
      : []),
    ...(image.alt ? [{ property: "og:image:alt", content: image.alt }] : []),
    ...(image.type ? [{ property: "og:image:type", content: image.type }] : []),
    { name: "twitter:card", content: "summary_large_image" },
    { name: "twitter:title", content: title },
    { name: "twitter:description", content: seo.description },
    { name: "twitter:image", content: image.url },
    ...(image.alt ? [{ name: "twitter:image:alt", content: image.alt }] : []),
    ...(article?.author
      ? [{ property: "article:author", content: article.author }]
      : []),
    ...(article?.publishedTime
      ? [{ property: "article:published_time", content: article.publishedTime }]
      : []),
    ...(article?.section
      ? [{ property: "article:section", content: article.section }]
      : []),
    ...(article?.tags ?? []).map((tag) => ({
      property: "article:tag",
      content: tag,
    })),
  ];
}

/** Meta for a missing post, category or author (R4-2): production's titles, robots noindex. */
export function blogNotFoundMeta(kind: BlogMissing) {
  return [
    {
      title: kind === "post" ? "Page not found | Decentraland" : "Decentraland",
    },
    { name: "robots", content: "noindex" },
  ];
}
