// from: shop@94584a5c:app/src/lib/ownership.ts
// You cannot buy your own listing. Two layers: early UX from the catalogue (a primary mint whose creator
// is the buyer), and the authoritative check right before money moves (the trade's signer is the buyer),
// which is right for primary and secondary alike. A creator buying a RESALE of their own creation is
// allowed: the catalogue's creator is not the reseller.

import type { AssetId } from "../../model/asset";

/** UI layer: a primary (item id, not a minted token) whose creator is the connected account. */
export function isOwnListing(item: { asset: Pick<AssetId, "token">; creator?: string | null }, address?: string | null): boolean {
  if (!address || !item.creator) return false;
  return item.asset.token.kind === "item" && item.creator.toLowerCase() === address.toLowerCase();
}

/** Authoritative layer: the trade's seller (signer) is the buyer. */
export function isOwnTrade(trade: { signer?: string | null }, buyer: string): boolean {
  return !!trade.signer && !!buyer && trade.signer.toLowerCase() === buyer.toLowerCase();
}
