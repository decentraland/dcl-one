// from: shop@94584a5c:app/src/lib/routes.ts
// Detail-page route builders. An item page (/shop/item/:contract/:itemId) sells a collection item; a
// token page (/shop/token/:contract/:tokenId) is one existing copy. Every link that opens a detail page
// goes through detailRouteFor, so a token never lands on the item route. Paths are absolute under
// /shop (upstream ran under a `/shop` basename; ui3 mounts the table as children of /shop).

import { SHOP_BASE } from "../../routes";

/** An href a plain anchor can use: Shop paths are already absolute, anything else gains the base. */
export function hrefFor(path: string): string {
  if (!path.startsWith("/") || path.startsWith("//")) throw new Error('hrefFor: path must start with a single "/"');
  return path === SHOP_BASE || path.startsWith(`${SHOP_BASE}/`) ? path : `${SHOP_BASE}${path}`;
}

export function itemRoute(contractAddress: string, itemId: string): string {
  return `${SHOP_BASE}/item/${contractAddress}/${itemId}`;
}

export function tokenRoute(contractAddress: string, tokenId: string): string {
  return `${SHOP_BASE}/token/${contractAddress}/${tokenId}`;
}

export function detailRouteFor(item: {
  contractAddress?: string | null;
  tokenId?: string | null;
  itemId?: string | null;
}): string | null {
  if (!item.contractAddress) return null;
  if (item.tokenId) return tokenRoute(item.contractAddress, item.tokenId);
  if (item.itemId) return itemRoute(item.contractAddress, item.itemId);
  return null;
}

/** Owner actions only on a token page, only for a token the viewer owns. */
export function canManageToken(opts: { isTokenRoute: boolean; ownsThisToken: boolean }): boolean {
  return opts.isTokenRoute && opts.ownsThisToken;
}

export const MY_CREATIONS = `${SHOP_BASE}/my-items?section=creations`;

/** NAMEs have their own Shop page (upstream's `/items?category=names`). */
export const NAMES_ROUTE = `${SHOP_BASE}/names`;

const OWNED_SECTIONS: Record<string, string | undefined> = { wearable: "wearables", emote: "emotes", ens: "names" };

/** My Items on the shelf that holds what was bought; a mixed basket keeps the default shelf. */
export function myItemsRouteFor(categories: Array<string | null | undefined>): string {
  const sections = new Set(categories.map((c) => OWNED_SECTIONS[c ?? ""]));
  const [only] = [...sections];
  return sections.size === 1 && only ? `${SHOP_BASE}/my-items?section=${only}` : `${SHOP_BASE}/my-items`;
}

/** A route segment with a query or fragment that leaked into the path (in-world `&utm_source`) cut off. */
export function routeSegment(raw?: string): string | undefined {
  return raw?.split(/[?&#]/)[0] || undefined;
}
