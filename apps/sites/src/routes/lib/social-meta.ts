import type { MetaDescriptor } from "react-router";

/**
 * Link-preview cards (Open Graph, Twitter) for every public page, rendered on the server so crawlers
 * that do not run JavaScript read them. The values match what decentraland.org's edge worker wrote
 * (sites-deployer `OpenGraphWriter` and its route handlers), with one `og:type` per page.
 */

export const SITE_NAME = "Decentraland";
export const SCHEMA_ORG = "https://schema.org";
export const TWITTER_HANDLE = "@decentraland";

/** The card image of every page without one of its own. */
export const DEFAULT_SOCIAL_IMAGE =
  "https://marketing-files.decentraland.org/uploads/1778186218133_decentraland-background.webp";
export const SOCIAL_LOGO = "https://marketing-files.decentraland.org/uploads/1778267186976_DCL_icon_fullcolor.png";

export const DEFAULT_SOCIAL_TITLE = "Decentraland — Come Hang Out";
export const DEFAULT_SOCIAL_DESCRIPTION =
  "Decentraland is where you hang out online. Join movie screenings, live music, trivia, and other community events. Show up anytime and see who's around.";

export type SocialCard = {
  /** The document title, used verbatim for `<title>`, `og:title` and `twitter:title`. */
  title: string;
  /** The card title when it differs from the document title (the home page). */
  socialTitle?: string;
  description?: string;
  /** Absolute image URL; the shared background when omitted. */
  image?: string;
  /**
   * `summary_large_image` unless the image is a small square (an avatar face or an item thumbnail),
   * which reads as `summary`.
   */
  smallImage?: boolean;
  /** Absolute canonical URL: `og:url` and `<link rel="canonical">`. */
  url?: string;
  type?: "website" | "article" | "profile" | "product";
  siteName?: string;
  /** Keeps the page out of the index and drops its canonical. */
  noindex?: boolean;
  /** schema.org objects, each in its own `application/ld+json` script. */
  jsonLd?: readonly object[];
  /** Tags specific to the page (`article:*`, `og:image:width`...), after the card. */
  extra?: readonly MetaDescriptor[];
};

/** The head of a public page: document title and description, the card, canonical and structured data. */
export function socialMeta(card: SocialCard): MetaDescriptor[] {
  const socialTitle = card.socialTitle ?? card.title;
  const description = card.description?.trim() || undefined;
  const image = card.image || DEFAULT_SOCIAL_IMAGE;
  const out: MetaDescriptor[] = [{ title: card.title }];
  if (description) out.push({ name: "description", content: description });
  out.push(
    { property: "og:title", content: socialTitle },
    { name: "twitter:title", content: socialTitle },
    { name: "twitter:site", content: TWITTER_HANDLE },
    { name: "twitter:creator", content: TWITTER_HANDLE },
    { property: "og:site_name", content: card.siteName ?? SITE_NAME },
  );
  if (description) {
    out.push({ property: "og:description", content: description }, { name: "twitter:description", content: description });
  }
  out.push(
    { property: "og:image", content: image },
    { name: "twitter:image", content: image },
    { name: "twitter:card", content: card.smallImage && card.image ? "summary" : "summary_large_image" },
    { property: "og:type", content: card.type ?? "website" },
  );
  if (card.noindex) {
    out.push({ name: "robots", content: "noindex" });
  } else if (card.url) {
    out.push({ property: "og:url", content: card.url }, { tagName: "link", rel: "canonical", href: card.url });
  }
  for (const data of card.jsonLd ?? []) out.push({ "script:ld+json": data });
  if (card.extra) out.push(...card.extra);
  return out;
}

/** The public origin pages name in canonical URLs, from the root loader. */
export function publicBase(matches: readonly ({ id: string; loaderData?: unknown } | undefined)[]): string | undefined {
  const root = matches.find((m) => m?.id === "root")?.loaderData as { publicPageBase?: string } | undefined;
  return root?.publicPageBase || undefined;
}

/** `base + path`, or undefined when the base is unknown (a canonical must be absolute). */
export function absoluteUrl(base: string | undefined, path: string): string | undefined {
  return base ? base.replace(/\/+$/, "") + path : undefined;
}

/**
 * An image a crawler can fetch: an http(s) URL as it is, a path of this site under `base`, else
 * undefined (the card then shows the shared image).
 */
export function absoluteImage(base: string | undefined, src: string | null | undefined): string | undefined {
  if (!src) return undefined;
  if (/^https?:\/\//i.test(src)) return src;
  return src.startsWith("/") && !src.startsWith("//") ? absoluteUrl(base, src) : undefined;
}

/** The head of a page whose entity does not exist: no card, out of the index. */
export function notFoundMeta(): MetaDescriptor[] {
  return [
    { title: "Page not found | Decentraland" },
    { name: "description", content: "The page you are looking for does not exist or has been moved." },
    { name: "robots", content: "noindex" },
  ];
}

/** Whitespace runs to one space, for descriptions written by users. */
export function oneLine(text: string | null | undefined): string | undefined {
  const out = text?.replace(/\s+/g, " ").trim();
  return out || undefined;
}

/** decentraland.org's shell card: the home page and every page without a card of its own. */
export function homeMeta(base: string | undefined): MetaDescriptor[] {
  return socialMeta({
    title: SITE_NAME,
    socialTitle: DEFAULT_SOCIAL_TITLE,
    description: DEFAULT_SOCIAL_DESCRIPTION,
    url: absoluteUrl(base, "/"),
    extra: [{ property: "og:logo", content: SOCIAL_LOGO }],
  });
}
