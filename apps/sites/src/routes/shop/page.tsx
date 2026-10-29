// One /shop route = one ui3 Shop page plus the SSR data sites adds: the document head and the public
// reads the server primed into the page's query cache. The server's data is used for the first paint
// and hydration only; after that the page reads for itself (client navigations skip the server loader),
// so a price on screen is always the page's own current read, never a stale server copy.

import { useState, type ReactNode } from "react";

import { SHOP_DEFAULT_TITLE, shopDocumentTitle } from "@ui/shop/discovery/hooks";
import { primeShopQuery, resetShopQueries } from "@ui/shop/discovery/query";
import type { ShopPageId } from "@ui/shop/routes";

import { defaultShopSeo, shopMetaDescriptors, type ShopPageData } from "@/data/lib/shop/pages";

const primed = new WeakSet<object>();

/** Seeds the page's queries from its loader data, once per loader answer, before the page reads them. */
export function primeShopPageData(data: ShopPageData | null | undefined): void {
  if (!data || primed.has(data)) return;
  primed.add(data);
  // The server's cache only ever holds primed answers; keeping just this document's bounds it. A page
  // reads its answers in the same synchronous render pass that primed them.
  if (typeof window === "undefined") resetShopQueries();
  for (const [key, value, at] of data.queries) primeShopQuery(key, value, at);
}

export function ShopPage({ data, children }: { data: ShopPageData | null | undefined; children: ReactNode }) {
  useState(() => primeShopPageData(data));
  return <>{children}</>;
}

/** Pages upstream keeps out of the index beyond the private ones: the event grid (Event.tsx noindex). */
const NOINDEX: ReadonlySet<ShopPageId> = new Set<ShopPageId>(["event"]);

/**
 * The document title as upstream Shop's useSeo writes it (shop@2d3bf62 app/src/hooks/useSeo.ts):
 * `<title> | Decentraland Shop`, the Overview as its default title. Same text as ui3's client-side
 * useSeo (discovery/hooks.ts shopDocumentTitle), so hydration never changes the tab title.
 */
export function shopTitle(page: ShopPageId, title: string): string {
  return page === "overview" || title === "Shop" ? SHOP_DEFAULT_TITLE : shopDocumentTitle(title);
}

/** The head of a Shop page: the loader's when it ran on the server, else the page's defaults. */
export function shopMeta(page: ShopPageId) {
  return ({ loaderData }: { loaderData?: unknown }) => {
    const seo = (loaderData as ShopPageData | null | undefined)?.seo ?? defaultShopSeo(page);
    const noindex = seo.noindex || NOINDEX.has(page);
    const title = shopTitle(page, seo.title);
    const out = shopMetaDescriptors({ ...seo, noindex, canonical: noindex ? undefined : seo.canonical }).map((d) => {
      if ("title" in d) return { title };
      if ("property" in d && d.property === "og:title") return { ...d, content: title };
      if ("property" in d && d.property === "og:site_name") return { ...d, content: "Decentraland Shop" };
      return d;
    });
    return out;
  };
}

/** Shop documents carry the visitor's session in their head (root data), so they are never shared. */
export function shopHeaders({ loaderHeaders, parentHeaders }: { loaderHeaders: Headers; parentHeaders: Headers }): Headers {
  const headers = new Headers(parentHeaders);
  headers.set("Cache-Control", loaderHeaders.get("Cache-Control") ?? "private, no-store");
  return headers;
}

/** Client navigations never wait on the server: the ui3 page fetches its own data. */
export async function shopClientLoader(): Promise<null> {
  return null;
}
