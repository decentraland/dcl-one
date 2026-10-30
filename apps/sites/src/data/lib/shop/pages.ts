// What the /shop routes tell crawlers and the first paint: per page, its title, description,
// whether it may be indexed, and the canonical URL. The page components themselves come from
// ui3/src/shop (SHOP.md "One implementation"); sites only adds SSR data and the document head.

import { SHOP_PAGES, type ShopPageId } from "@ui/shop/routes";

/** A primed answer for the page's own query cache: its key, the data, when the server read it. */
export type PrimedQuery = [key: readonly unknown[], data: unknown, at: number];

export type ShopSeo = {
  title: string;
  description: string;
  image?: string;
  /** Absolute URL of the page without its query (the canonical identity of the asset or list). */
  canonical?: string;
  noindex: boolean;
};

export type ShopPageData = {
  page: ShopPageId;
  seo: ShopSeo;
  queries: PrimedQuery[];
  /** The session id the page's own telemetry is tagged with (set by the route loader). */
  sid?: string;
};

const SITE = "Decentraland";

const DESCRIPTIONS: Partial<Record<ShopPageId, string>> = {
  overview: "Shop Decentraland wearables, emotes, NAMEs and LAND: featured drops, trending items and the latest listings.",
  items: "Browse Decentraland wearables and emotes. Filter by category, rarity and price.",
  event: "The items of the running Decentraland shop event.",
  lands: "Buy, sell and rent Decentraland LAND parcels and estates on the map or in a list.",
  names: "Claim a unique Decentraland NAME, or buy one another player is selling.",
  rankings: "The best-selling wearables, emotes, creators and collectors in Decentraland.",
  credits: "Get Decentraland credits and use them on eligible wearables, emotes and NAMEs.",
};

/** Pages about the visitor's own account, wallet or an in-progress purchase: never indexed. */
const PRIVATE: ReadonlySet<ShopPageId> = new Set<ShopPageId>([
  "myItems",
  "myStore",
  "myFavorites",
  "favoriteList",
  "activity",
  "cart",
  "success",
  "settings",
  "storeSettings",
  "outfitStudioManage",
  "outfitStudioNew",
  "outfitStudioEdit",
  "tokenSell",
  "tokenCancel",
  "tokenTransfer",
  "tokenBid",
  "tokenBids",
  "tokenManage",
  "tokenRent",
  "tokenBuyStatus",
  "itemBid",
  "itemBuyStatus",
  "notFound",
]);

export function shopPageTitle(page: ShopPageId): string {
  return SHOP_PAGES.find((p) => p.id === page)?.title ?? "Shop";
}

export function isPrivateShopPage(page: ShopPageId): boolean {
  return PRIVATE.has(page);
}

/** The head for `page` when the loader found nothing more specific. */
export function defaultShopSeo(page: ShopPageId, canonical?: string): ShopSeo {
  return {
    title: page === "overview" ? "Shop" : shopPageTitle(page),
    description: DESCRIPTIONS[page] ?? "The Decentraland Shop: wearables, emotes, NAMEs and LAND.",
    canonical: isPrivateShopPage(page) ? undefined : canonical,
    noindex: isPrivateShopPage(page),
  };
}

/** The document title: "<page> | Shop | Decentraland", the overview as "Shop | Decentraland". */
export function shopDocumentTitle(seo: Pick<ShopSeo, "title">): string {
  return seo.title === "Shop" ? `Shop | ${SITE}` : `${seo.title} | Shop | ${SITE}`;
}

export type MetaDescriptor =
  | { title: string }
  | { name: string; content: string }
  | { property: string; content: string }
  | { tagName: "link"; rel: string; href: string };

/** react-router meta descriptors for a Shop page head. */
export function shopMetaDescriptors(seo: ShopSeo): MetaDescriptor[] {
  const title = shopDocumentTitle(seo);
  const out: MetaDescriptor[] = [
    { title },
    { name: "description", content: seo.description },
    { property: "og:title", content: title },
    { property: "og:description", content: seo.description },
    { property: "og:type", content: "website" },
    { property: "og:site_name", content: SITE },
    { name: "twitter:card", content: seo.image ? "summary_large_image" : "summary" },
  ];
  if (seo.image) out.push({ property: "og:image", content: seo.image });
  if (seo.canonical) {
    out.push({ property: "og:url", content: seo.canonical });
    out.push({ tagName: "link", rel: "canonical", href: seo.canonical });
  }
  if (seo.noindex) out.push({ name: "robots", content: "noindex" });
  return out;
}
