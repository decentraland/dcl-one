// from: shop@94584a5c:app/src/lib/manage-cache.ts
// Optimistically patch every cache that renders an owned token's sale state the moment a manage action
// confirms (list, reprice, remove, transfer), without waiting for the feed's eventually-consistent view.
// Invalidation still runs beside it for the authoritative answer. One change from upstream: a token's
// listing is a resale, so the patch carries the signed MANA price, never a credit price.

import type { InfiniteData, QueryClient } from "@tanstack/react-query";

import type { ListingPrice, MyAsset } from "../../market/lib/api";

export type ManageTarget = { address?: string; contractAddress: string; tokenId?: string };

/** listed: on sale at `price` (a fresh listing or a new price); removed: delisted; gone: transferred away. */
export type ManagePatch = { kind: "listed"; price: ListingPrice; tradeId?: string } | { kind: "removed" } | { kind: "gone" };

type MyAssetsPage = { items: MyAsset[]; total: number };
type SecondarySaleMap = Record<string, { price: ListingPrice; tradeId: string }>;

function applyToAsset(asset: MyAsset, patch: ManagePatch): MyAsset {
  if (patch.kind === "listed") {
    return {
      ...asset,
      isOnSale: true,
      listing: { tradeId: patch.tradeId ?? asset.listing?.tradeId ?? null, orderId: asset.listing?.orderId ?? "", price: patch.price },
    };
  }
  return { ...asset, isOnSale: false, listing: undefined };
}

const same = (a: MyAsset, contractAddress: string, tokenId: string) => a.contractAddress.toLowerCase() === contractAddress.toLowerCase() && a.tokenId === tokenId;

/** Upstream `patchManageCaches`, over the owned-token, my-assets and secondary-sale-state query keys. */
export function patchManageCaches(qc: QueryClient, target: ManageTarget, patch: ManagePatch): void {
  const { address, contractAddress, tokenId } = target;
  if (!contractAddress || !tokenId) return;
  const key = `${contractAddress.toLowerCase()}-${tokenId}`;
  const gone = patch.kind === "gone";

  void qc.cancelQueries({ queryKey: ["owned-token", contractAddress, tokenId] });
  void qc.cancelQueries({ queryKey: ["secondary-sale-state"] });
  if (address) void qc.cancelQueries({ queryKey: ["my-assets", address] });

  qc.setQueriesData<MyAsset | null>({ queryKey: ["owned-token", contractAddress, tokenId] }, (prev) => {
    if (!prev) return prev;
    return gone ? null : applyToAsset(prev, patch);
  });

  qc.setQueriesData<InfiniteData<MyAssetsPage>>({ queryKey: address ? ["my-assets", address] : ["my-assets"] }, (prev) => {
    if (!prev?.pages) return prev;
    return {
      ...prev,
      pages: prev.pages.map((page) => {
        if (gone) {
          const items = page.items.filter((a) => !same(a, contractAddress, tokenId));
          return { ...page, items, total: Math.max(0, page.total - (page.items.length - items.length)) };
        }
        return { ...page, items: page.items.map((a) => (same(a, contractAddress, tokenId) ? applyToAsset(a, patch) : a)) };
      }),
    };
  });

  qc.setQueriesData<SecondarySaleMap>({ queryKey: ["secondary-sale-state"] }, (prev) => {
    if (!prev) return prev;
    const next = { ...prev };
    if (patch.kind === "listed") next[key] = { price: patch.price, tradeId: patch.tradeId ?? next[key]?.tradeId ?? "" };
    else delete next[key];
    return next;
  });
}
