import type { OfferSource } from "../detail/offers";
import { sameAsset } from "../model/asset";
import { decodePurchaseOffer } from "../model/wire";
import { ENDPOINTS } from "./endpoints";
import type { ShopClient } from "./shop";

/** The node quotes each line in request order; prices and eligibility never come from the cart. */
export function purchaseOfferSource(client: ShopClient): OfferSource {
  return async (queries, session, signal) => {
    if (!queries.length) return {};
    if (queries.length > 50 || new Set(queries.map(q => q.key)).size !== queries.length) throw new Error("Invalid purchase quote request.");
    const response = await client.api.request<{ offers?: unknown }>(ENDPOINTS.purchaseOffers, {
      body: { lines: queries.map(({ asset, listing, quantity }) => ({ asset, listing, quantity: quantity ?? 1 })) },
      metadata: { walletKind: session.kind === "web3" ? "web3" : "web2" },
      signal,
    });
    if (!response || !Array.isArray(response.offers) || response.offers.length !== queries.length) throw new Error("Incomplete purchase quote response.");
    return Object.fromEntries(response.offers.map((answer: unknown, index: number) => {
      const query = queries[index]!;
      const error = answer as { index?: unknown; error?: unknown; message?: unknown } | null;
      if (error && typeof error.error === "string") {
        if (error.index !== index) throw new Error("Purchase quote error refers to another line.");
        if (error.error === "Bad Request" || error.error === "payment_rule_refused") return [query.key, null];
        throw new Error(typeof error.message === "string" ? error.message : "Purchase quote unavailable.");
      }
      const offer = decodePurchaseOffer(answer);
      if (!offer || !sameAsset(offer.asset, query.asset) || !Number.isSafeInteger(offer.quotedAt) || offer.quotedAt <= 0) throw new Error("Invalid purchase quote response.");
      return [query.key, offer];
    }));
  };
}
