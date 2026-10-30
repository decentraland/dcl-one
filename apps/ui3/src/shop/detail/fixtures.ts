// Server offers for the detail tests and stories, built by payment/fixtures.ts the way
// tests/shop/policy.rs builds its facts, for the same assets discovery/fixtures.ts lists: item 7 of the
// collection (a store mint / creator order), token 42 (its resale), a LAND parcel and a NAME. An
// `outage` variant keeps the rule's methods but states the credits price missing, as the server does
// when the MANA/USD feed is stale.

import type { PurchaseOffer } from "../model/purchase";
import type { Session } from "../model/settlement";
import { creatorOrder, nameRegistration, parcelResale, storeMint, wearableResale } from "../payment/fixtures";
import type { OfferSource } from "./offers";

export type OfferKind = "storeMint" | "creatorOrder" | "resale" | "parcel" | "name";

const BUILD: Record<OfferKind, (session: Session) => PurchaseOffer> = {
  storeMint: (s) => storeMint(s),
  creatorOrder: (s) => creatorOrder(s),
  resale: (s) => wearableResale(s),
  parcel: (s) => parcelResale(s),
  name: (s) => nameRegistration(s),
};

/** The server stating the credits price missing (the MANA/USD feed): credits stay, unpriced. */
export function withCreditsOutage(offer: PurchaseOffer): PurchaseOffer {
  if (!offer.eligibility.allowed.includes("credits")) return offer;
  return { ...offer, quotes: offer.quotes.filter((q) => q.method !== "credits"), unavailable: [{ method: "credits", missing: "manaUsdRate" }] };
}

export type OfferScenario = { kind: OfferKind; outage?: boolean } | "routeMissing" | "failed" | "loading";

/** An offer source answering every query with the scenario's offer for the asking session. */
export function fixtureOfferSource(scenario: OfferScenario): OfferSource & { calls: number } {
  const source = Object.assign(
    async (queries: Parameters<OfferSource>[0], session: Session) => {
      source.calls++;
      if (scenario === "loading") return new Promise<never>(() => {});
      if (scenario === "failed") throw new Error("quote route answered 500");
      if (scenario === "routeMissing") {
        const { OfferRouteMissing } = await import("./offers");
        throw new OfferRouteMissing("no route");
      }
      const offer = BUILD[scenario.kind](session);
      const answer = scenario.outage ? withCreditsOutage(offer) : offer;
      return Object.fromEntries(queries.map((q) => [q.key, answer]));
    },
    { calls: 0 },
  );
  return source;
}
