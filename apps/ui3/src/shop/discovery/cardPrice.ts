// from: shop@94584a5c:app/src/components/AssetCard/AssetCard.tsx (the price branch), app/src/lib/mana-convert.ts (displayCredits)
// What price a browse card may SHOW. A card has no server offer yet (the detail page and the cart ask
// the server for one), so it reads the feed row under the payment rule's presentation mirror
// (payment/rule.ts) in its MOST RESTRICTIVE reading: a row that identifies an existing token, or that
// the feed labels a resale, is judged a resale; anything not a Decentraland wearable or emote, or not
// on Polygon, is MANA only. So a card never shows a credit price, a top-up or a credits+MANA option for
// LAND or a resale, and never switches an eligible item to MANA when its credit price is missing:
// that is an explicit "Credits price unavailable". Upstream converted MANA rows to credits at a live
// rate; here the credit figure is the server's `priceCredits` and MANA is shown in exact units.

import { itemAsset, tokenAsset, type AssetKind } from "../model/asset";
import { parseEvmAddress } from "../model/address";
import { parseManaWei, type ManaWei } from "../model/money";
import type { Reason, Session } from "../model/settlement";
import { ruleFor } from "../payment/rule";
import type { CatalogItem } from "./lib/catalogItem";

export type CardPrice =
  /** Eligible primary: credits are the price; a web3 wallet also sees the exact MANA when the row has one. */
  | { kind: "credits"; credits: number; compareAt: number | null; mana: ManaWei | null }
  /** Eligible primary whose credit price the feed did not give: an outage, stated as such. */
  | { kind: "creditsUnavailable"; mana: ManaWei | null }
  /** MANA only, exact. */
  | { kind: "mana"; mana: ManaWei; reasons: Reason[] }
  /** MANA only, priced in USD and settled in MANA at a current quote: the exact amount is the checkout's. */
  | { kind: "manaAtCheckout"; reasons: Reason[] }
  | { kind: "soldOut" }
  | { kind: "notForSale" };

export function assetKindOf(category: string): AssetKind {
  switch (category) {
    case "wearable":
      return "wearable";
    case "emote":
      return "emote";
    case "ens":
    case "name":
      return "name";
    case "parcel":
      return "parcel";
    case "estate":
      return "estate";
    default:
      return "other";
  }
}

/** The presentation mirror for one row, read restrictively; null when the row does not identify an asset. */
export function cardEligibility(item: CatalogItem, session: Session) {
  const contract = parseEvmAddress(item.contractAddress);
  if (!contract || !Number.isSafeInteger(item.chainId) || item.chainId <= 0) return null;
  const resale = !!item.tokenId || item.listingType === "secondary";
  let asset;
  try {
    asset = item.tokenId
      ? tokenAsset(item.chainId, contract, item.tokenId)
      : item.itemId != null
        ? itemAsset(item.chainId, contract, item.itemId)
        : null;
  } catch {
    return null;
  }
  if (!asset) return null;
  return ruleFor({ asset, kind: assetKindOf(item.category), sale: resale ? "secondary" : "primaryMint", bundle: false, settlement: [] }, session);
}

export function cardPrice(item: CatalogItem, session: Session): CardPrice {
  const listed = item.onSale !== false && (item.priceCredits > 0 || !!item.manaWei || !!item.tradeId || item.acquisition === "store");
  const primary = !item.tokenId && item.listingType !== "secondary";
  if (primary && item.available === 0) return { kind: "soldOut" };
  if (!listed) return { kind: "notForSale" };
  const rule = cardEligibility(item, session);
  const mana = parseManaWei(item.manaWei ?? null);
  if (rule && rule.allowed.includes("credits")) {
    const showMana = session.kind === "web3" ? mana : null;
    if (item.priceCredits > 0) {
      const compareAt = item.compareAtCredits != null && item.compareAtCredits > item.priceCredits ? item.compareAtCredits : null;
      return { kind: "credits", credits: item.priceCredits, compareAt, mana: showMana };
    }
    return { kind: "creditsUnavailable", mana: showMana };
  }
  const reasons = rule?.reasons ?? ["unsupportedCombination"];
  return mana !== null && mana > 0n ? { kind: "mana", mana, reasons } : { kind: "manaAtCheckout", reasons };
}
