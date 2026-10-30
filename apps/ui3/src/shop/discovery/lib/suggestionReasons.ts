// from: shop@94584a5c:app/src/lib/suggestionReasons.ts
import type { SuggestionReasonKind } from "@catalyst-api/market/SuggestionReasonKind";
import { itemRoute } from "./routes";

const REASON_KEYS: Record<SuggestionReasonKind, string> = {
  co_owned: "overview.suggested.reason.coOwned",
  creator_affinity: "overview.suggested.reason.creatorAffinity",
  favorite_similar: "overview.suggested.reason.favoriteSimilar",
  equipped_similar: "overview.suggested.reason.equippedSimilar",
  seed_similar: "overview.suggested.reason.seedSimilar",
  trending: "overview.suggested.reason.trending",
};

export function reasonKey(kind: SuggestionReasonKind): string | undefined {
  return REASON_KEYS[kind];
}

/** Only "Because you have {item}" names the item, so only it needs the name resolved. */
export function reasonInterpolatesItemName(kind: SuggestionReasonKind): boolean {
  return kind === "co_owned";
}

/** Every kind the server attaches an item to links to it. */
export function reasonLinksToItem(kind: SuggestionReasonKind): boolean {
  return kind !== "creator_affinity" && kind !== "trending";
}

/** `contract-itemId` -> the item page, or null when the id is not one the Shop routes. */
export function triggerItemPath(triggerId: string): string | null {
  const split = triggerId.lastIndexOf("-");
  if (split <= 0) return null;
  const contractAddress = triggerId.slice(0, split);
  const itemId = triggerId.slice(split + 1);
  if (!contractAddress.startsWith("0x") || !/^\d+$/.test(itemId)) return null;
  return itemRoute(contractAddress, itemId);
}
