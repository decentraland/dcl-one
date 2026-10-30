// The purchase model the Shop UI renders and the cart keeps. PurchaseOffer is the server's answer
// for one line: every field is a serde form of a src/shop/policy.rs value (the line's derived
// `Authoritative` kind and sale, `eligibility`, `quote` per offered method, the settlement key each
// method joins), so the UI renders what the server decided and computes nothing it could trust itself.

import type { AssetId, AssetKind, SaleKind } from "./asset";
import type { ManaWei, UsdCents } from "./money";
import type {
  Eligibility,
  MissingFact,
  PaymentMethod,
  Quote,
  SettlementGroupKey,
} from "./settlement";

export type PurchaseOffer = {
  asset: AssetId;
  /** `Authoritative::kind`, derived by the server from the registry and the index. */
  kind: AssetKind;
  /** `Authoritative::sale`, derived by the server from the decoded offer. */
  sale: SaleKind;
  /** Several assets in one signed trade. */
  bundle: boolean;
  /** `policy::eligibility` for the session that asked. */
  eligibility: Eligibility;
  /** `policy::quote` for each method the server could price. */
  quotes: Quote[];
  /** Methods the rule offers that cannot be priced right now (an outage), with the missing fact. */
  unavailable: { method: PaymentMethod; missing: MissingFact }[];
  /** The confirmation group each offered method settles in. */
  settlement: { method: PaymentMethod; key: SettlementGroupKey }[];
  /** Remaining mintable supply for a primary mint; null when unknown or not a mint. */
  available: number | null;
  /** When the server answered (epoch ms). */
  quotedAt: number;
};

export function offerQuote(offer: PurchaseOffer, method: PaymentMethod): Quote | null {
  return offer.quotes.find((q) => q.method === method) ?? null;
}

export function offerSettlementKey(offer: PurchaseOffer, method: PaymentMethod): SettlementGroupKey | null {
  return offer.settlement.find((s) => s.method === method)?.key ?? null;
}

/** How a line is acquired, as upstream `ListingAcquisition` (absent means a trade). */
export type ListingAcquisition = "trade" | "store";

/** What identifies the listing a cart line buys, beyond the asset: never a price. */
export type CartListingRef = {
  tradeId?: string;
  acquisition?: ListingAcquisition;
  orderId?: string;
  rentalId?: string;
  /** A NAME registration's label. */
  name?: string;
  couponId?: string;
};

/** A rendering snapshot taken when the line was added; never a price authority. */
export type CartLineDisplay = {
  name: string;
  thumbnail?: string;
  category?: string;
  rarity?: string;
  creator?: string;
  urn?: string;
  href?: string;
};

/** Upstream `AddToCartSource` (first touch wins). */
export type CartProvenance = { source?: string; outfitId?: string };

export type CartLine = {
  /** Stable key: the asset plus the listing identity. */
  id: string;
  asset: AssetId;
  /** More than one only for a primary mint (an item ref); an existing token is unique. */
  quantity: number;
  /** The buyer's choice; null means the presentation default. Never rewritten by the app. */
  method: PaymentMethod | null;
  listing: CartListingRef;
  display: CartLineDisplay;
  provenance: CartProvenance;
  /** The last authoritative offer the buyer was shown, for revalidation diffs. */
  seen: PurchaseOffer | null;
  addedAt: number;
};

/** idle -> authorizing -> submitted -> pending -> confirmed | failed | expired. */
export type GroupStatus = "idle" | "authorizing" | "submitted" | "pending" | "confirmed" | "failed" | "expired";

export type GroupFailure = {
  /** rejected: the wallet declined; refused: the server refused; reverted: the chain reverted; error: anything else before a broadcast. */
  kind: "rejected" | "refused" | "reverted" | "error";
  message: string;
};

/** One confirmation group's progress inside a checkout. */
export type GroupProgress = {
  keyId: string;
  key: SettlementGroupKey;
  /** Cart line ids the group settles, in cart order. */
  lineIds: string[];
  status: GroupStatus;
  /** Credit salts authorized for the group (credits groups). */
  creditIds: string[];
  txHash: string | null;
  failure: GroupFailure | null;
  /** Epoch ms of the last transition. */
  updatedAt: number;
  /** How many times the group was started; a resume increments it. */
  attempts: number;
};

export type PurchaseState = {
  id: string;
  /** Lowercase account the checkout belongs to. */
  account: string;
  groups: GroupProgress[];
  createdAt: number;
};

export type CartTotals = {
  credits: { quoted: UsdCents; charged: UsdCents };
  mana: ManaWei;
};
