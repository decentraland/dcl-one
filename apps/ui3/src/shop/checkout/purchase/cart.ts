// from: shop@94584a5c:app/src/lib/cart-availability.ts, app/src/lib/cart-checkout.ts
// What a cart line settles as, read live right before money moves. Upstream priced each line from its
// live trade on the client; here the server's PurchaseOffer (cart/revalidate) is the price, stock and
// eligibility authority, and this module only resolves the SETTLEMENT material the transaction needs
// (the signed trade to accept, the coupon to redeem, the mint's live store price) and checks that it
// still agrees with the offer the buyer confirmed. Anything that disagrees is `stale` (re-quote), never
// silently charged; a line whose settlement this cart cannot build (a NAME registration, a rental, a
// legacy order: single-asset calls) is `unsupported` and finishes on its own page.

import { shopClient, type ShopClient } from "../../api/shop";
import type { CreditLineRequest } from "../../api/types";
import { tokenRefId } from "../../model/asset";
import { discountedCents, discountedMana, usdCents, type ManaWei, type UsdCents } from "../../model/money";
import { offerQuote, offerSettlementKey, type CartLine, type PurchaseOffer } from "../../model/purchase";
import type { PaymentMethod, Quote, Session } from "../../model/settlement";
import { presentPayment } from "../../payment/present";
import { lineUnits } from "../../cart/partition";
import { fetchStoreMintState, isTradeNotFound, resolveLiveCoupon, resolveLiveTrade, usdWeiToCents } from "../lib/api";
import { getCouponManagerForTrade, getMarketplaceForTrade } from "../lib/marketplace";
import { isOwnTrade } from "../lib/ownership";
import { ASSET_TYPE, type ListingCoupon, type SignedTrade } from "./encoding";
import type { PurchaseTarget } from "./mana";

const PPM = 1_000_000;
/** CollectionDiscountCoupon.DISCOUNT_TYPE_RATE: the only kind the Shop prices and settles. */
const RATE_DISCOUNT = 1;

/** sessionStorage flag: a top-up was started to finish this cart; the cart page resumes the review on return. */
export const RESUME_CART_KEY = "dcl_shop_resume_cart";

// ---- availability (cart-availability.ts) ----

export type CartLineAvailability = "available" | "sold-out" | "unavailable";

/** Optimistic: a line is buyable until its live listing is proven gone. */
export function isLineBuyable(status: CartLineAvailability | undefined): boolean {
  return status === undefined || status === "available";
}

/** A line's live trade: no trade is sold out (a mint) or unavailable (a token); expired or $0 is unavailable. */
export function classifyTrade(line: Pick<CartLine, "asset">, trade: SignedTrade | null, now: number = Date.now()): CartLineAvailability {
  if (!trade) return line.asset.token.kind === "token" ? "unavailable" : "sold-out";
  if (!getMarketplaceForTrade(trade)) return "unavailable";
  const expiration = trade.checks?.expiration;
  if (typeof expiration === "number" && Number.isFinite(expiration) && expiration > 0) {
    const ms = expiration > 1e12 ? expiration : expiration * 1000;
    if (ms <= now) return "unavailable";
  }
  const price = trade.received?.[0];
  const amount = price?.amount ?? price?.value;
  const positive = (() => {
    try {
      return !!amount && BigInt(amount) > 0n;
    } catch {
      return false;
    }
  })();
  return positive ? "available" : "unavailable";
}

/** A mint's live store state: only remaining stock and a positive price matter. */
export function classifyStoreMint(state: { priceWei: string; available: number } | null): CartLineAvailability {
  if (!state || state.available <= 0) return "sold-out";
  try {
    return BigInt(state.priceWei) > 0n ? "available" : "unavailable";
  } catch {
    return "unavailable";
  }
}

function itemIdOf(line: CartLine): string | null {
  return line.asset.token.kind === "item" ? line.asset.token.itemId : null;
}

/** A line's availability through the same resolvers checkout uses. Network failures propagate (stay optimistic). */
export async function resolveLineAvailability(line: CartLine, client: ShopClient = shopClient()): Promise<CartLineAvailability> {
  const itemId = itemIdOf(line);
  if (line.listing.acquisition === "store") {
    return itemId ? classifyStoreMint(await fetchStoreMintState(line.asset.contract, itemId, client)) : "unavailable";
  }
  try {
    const trade = await resolveLiveTrade({ tradeId: line.listing.tradeId, contractAddress: line.asset.contract, itemId }, client);
    const status = classifyTrade(line, trade);
    return status === "sold-out" && itemId ? classifyStoreMint(await fetchStoreMintState(line.asset.contract, itemId, client)) : status;
  } catch (e) {
    if (isTradeNotFound(e)) {
      if (line.asset.token.kind === "token" || !itemId) return "unavailable";
      return classifyStoreMint(await fetchStoreMintState(line.asset.contract, itemId, client));
    }
    throw e;
  }
}

// ---- checkout (cart-checkout.ts) ----

export type LineSettlement =
  | { acquisition: "trade"; trade: SignedTrade; coupon?: ListingCoupon }
  | { acquisition: "store"; priceWei: string; available: number };

export type ResolvedLine = {
  line: CartLine;
  offer: PurchaseOffer;
  method: PaymentMethod;
  /** The server's unit quote for the method: what a credits line authorizes and what MANA sizes to. */
  quote: Quote;
  quantity: number;
} & LineSettlement;

/** The purchase a resolved line settles as, minus the credit that pays for it. */
export type PurchaseDraft =
  | { kind: "store"; item: { collection: string; itemId: string; priceWei: string }; chainId: number }
  | { kind: "trade"; trade: SignedTrade; coupon?: ListingCoupon };

export function draftPurchase(resolved: ResolvedLine): PurchaseDraft {
  if (resolved.acquisition === "store") {
    return {
      kind: "store",
      item: { collection: resolved.line.asset.contract, itemId: tokenRefId(resolved.line.asset.token), priceWei: resolved.priceWei },
      chainId: resolved.line.asset.chainId,
    };
  }
  return { kind: "trade", trade: resolved.trade, ...(resolved.coupon ? { coupon: resolved.coupon } : {}) };
}

/**
 * The line the batch authorization signs: the SERVER's quoted cents (never a client price), the trade
 * it settles when there is one, and the item identity so the purchase history can name it.
 */
export function checkoutLineFor(resolved: ResolvedLine): CreditLineRequest {
  if (resolved.quote.method !== "credits") throw new Error("Only a credits line is authorized");
  const itemId = itemIdOf(resolved.line);
  return {
    usdPriceCents: resolved.quote.quoted,
    ...(resolved.acquisition === "trade" ? { tradeId: resolved.trade.id } : {}),
    ...(itemId ? { contractAddress: resolved.line.asset.contract, itemId } : {}),
  };
}

export function purchaseTargetFor(resolved: ResolvedLine): PurchaseTarget {
  if (resolved.acquisition === "trade") return { kind: "trade", trade: resolved.trade, ...(resolved.coupon ? { coupon: resolved.coupon } : {}) };
  const item = { collection: resolved.line.asset.contract, itemId: tokenRefId(resolved.line.asset.token), priceWei: resolved.priceWei };
  return { kind: "store", mint: { item, chainId: resolved.line.asset.chainId } };
}

/** One unit per copy: N copies of a mint are N units of the same group (the trade's uses allow it). */
export function toUnits(lines: ResolvedLine[]): ResolvedLine[] {
  return lines.flatMap((l) => Array.from({ length: l.quantity }, () => ({ ...l, quantity: 1 })));
}

/** Split units into the groups that each become one transaction and one credit, in cart order. */
export function groupUnitsForAuthorization(units: ResolvedLine[], keyOf: (draft: PurchaseDraft) => string): { key: string; units: ResolvedLine[]; drafts: PurchaseDraft[] }[] {
  const groups = new Map<string, { key: string; units: ResolvedLine[]; drafts: PurchaseDraft[] }>();
  for (const unit of units) {
    const draft = draftPurchase(unit);
    const key = keyOf(draft);
    const existing = groups.get(key);
    if (existing) {
      existing.units.push(unit);
      existing.drafts.push(draft);
    } else groups.set(key, { key, units: [unit], drafts: [draft] });
  }
  return [...groups.values()];
}

/**
 * The coupon a trade can actually settle with, or undefined: a rate discount inside its window, on
 * COLLECTION_ITEM assets only, signed against the manager of the marketplace this trade settles on.
 * Anything else reverts the whole purchase after the buyer confirmed.
 */
export function couponForTrade(coupon: ListingCoupon | undefined, trade: SignedTrade, now: number = Date.now()): ListingCoupon | undefined {
  if (!coupon || coupon.discountType !== RATE_DISCOUNT) return undefined;
  if (coupon.discount <= 0 || coupon.discount >= PPM) return undefined;
  const ms = (v: number) => (v > 1e12 ? v : v * 1000);
  if (ms(Number(coupon.checks?.expiration)) <= now || !(ms(Number(coupon.checks?.effective)) <= now)) return undefined;
  const manager = getCouponManagerForTrade(trade);
  if (manager === null || coupon.couponManager?.toLowerCase() !== manager) return undefined;
  const sent = trade.sent ?? [];
  return sent.length > 0 && sent.every((a) => Number(a.assetType) === ASSET_TYPE.COLLECTION_ITEM) ? coupon : undefined;
}

export type TradeResolver = (line: CartLine) => Promise<SignedTrade | null>;
export type StoreResolver = (line: CartLine) => Promise<{ priceWei: string; available: number } | null>;
export type CouponResolver = (line: CartLine) => Promise<ListingCoupon | undefined>;

export type SettlementResolvers = { trade: TradeResolver; store: StoreResolver; coupon?: CouponResolver };

/** The live resolvers over the one client. */
export function liveResolvers(client: ShopClient = shopClient()): SettlementResolvers {
  return {
    trade: (line) => resolveLiveTrade({ tradeId: line.listing.tradeId, contractAddress: line.asset.contract, itemId: itemIdOf(line) }, client),
    store: async (line) => {
      const itemId = itemIdOf(line);
      return itemId ? fetchStoreMintState(line.asset.contract, itemId, client) : null;
    },
    coupon: (line) => resolveLiveCoupon({ contractAddress: line.asset.contract, itemId: itemIdOf(line) }, client),
  };
}

/** Why a line cannot be charged now. */
export type LineUnbuyable = "gone" | "own" | "no-price" | "stale" | "unsupported";

export type LineOutcome = { status: "buyable"; resolved: ResolvedLine } | { status: LineUnbuyable; lineId: string; detail?: string };

function tradePriceMatches(trade: SignedTrade, coupon: ListingCoupon | undefined, quote: Quote): boolean {
  const price = trade.received[0];
  const amount = price?.amount ?? price?.value;
  if (!price || !amount) return false;
  if (quote.method === "credits") {
    if (price.assetType !== ASSET_TYPE.USD_PEGGED_MANA) return true;
    const list = usdWeiToCents(amount);
    const expected = coupon ? discountedCents(usdCents(list), coupon.discount) : usdCents(list);
    return expected === quote.quoted;
  }
  if (price.assetType !== ASSET_TYPE.ERC20) return true;
  const wei = BigInt(amount) as ManaWei;
  return (coupon ? discountedMana(wei, coupon.discount) : wei) === quote.amount;
}

/**
 * Resolve ONE line for settlement against the offer the buyer confirmed. Reads only; nothing is
 * reserved. `method` is the buyer's choice (or the presentation default), never a re-routed one.
 */
export async function resolveLine(
  line: CartLine,
  offer: PurchaseOffer,
  session: Session,
  buyer: string,
  resolvers: SettlementResolvers,
  now: number = Date.now(),
): Promise<LineOutcome> {
  const view = presentPayment(offer, session, { report: () => {} });
  const method = line.method ?? view.primary;
  const quote = offerQuote(offer, method);
  const key = offerSettlementKey(offer, method);
  if (!view.options.some((o) => o.method === method) || !quote || !key) return { status: "no-price", lineId: line.id };
  const quantity = lineUnits(line);
  if (offer.available !== null && quantity > offer.available) return { status: "gone", lineId: line.id };
  const base = { line, offer, method, quote, quantity };

  if (key.call.kind === "one") return { status: "unsupported", lineId: line.id, detail: offer.kind };
  if (key.call.kind === "storeBuy") {
    const live = await resolvers.store(line);
    if (!live || live.available < quantity) return { status: "gone", lineId: line.id };
    if (quote.method === "mana" && BigInt(live.priceWei) !== quote.amount) return { status: "stale", lineId: line.id, detail: "store price moved" };
    return { status: "buyable", resolved: { ...base, acquisition: "store", priceWei: live.priceWei, available: live.available } };
  }

  const trade = await resolvers.trade(line);
  if (!trade) return { status: "gone", lineId: line.id };
  const market = getMarketplaceForTrade(trade);
  if (!market) return { status: "gone", lineId: line.id };
  if (market.address !== key.contract || trade.chainId !== key.chainId) return { status: "stale", lineId: line.id, detail: "settles elsewhere" };
  if (isOwnTrade(trade, buyer)) return { status: "own", lineId: line.id };
  let coupon: ListingCoupon | undefined;
  if (key.call.kind === "acceptWithCoupon") {
    coupon = couponForTrade(await resolvers.coupon?.(line), trade, now);
    if (!coupon) return { status: "stale", lineId: line.id, detail: "the sale ended" };
  }
  if (!tradePriceMatches(trade, coupon, quote)) return { status: "stale", lineId: line.id, detail: "price moved" };
  return { status: "buyable", resolved: { ...base, acquisition: "trade", trade, ...(coupon ? { coupon } : {}) } };
}

export type CartReview = {
  buyable: ResolvedLine[];
  unavailable: string[];
  own: string[];
  stale: string[];
  unsupported: string[];
  noPrice: string[];
};

/** Resolve every line; one bad line never aborts the basket (it lands in `unavailable`). Sequential on purpose. */
export async function reviewCart(
  priced: { line: CartLine; offer: PurchaseOffer | null }[],
  session: Session,
  buyer: string,
  resolvers: SettlementResolvers,
  now: number = Date.now(),
): Promise<CartReview> {
  const review: CartReview = { buyable: [], unavailable: [], own: [], stale: [], unsupported: [], noPrice: [] };
  for (const { line, offer } of priced) {
    if (!offer) {
      review.unavailable.push(line.id);
      continue;
    }
    try {
      const outcome = await resolveLine(line, offer, session, buyer, resolvers, now);
      if (outcome.status === "buyable") review.buyable.push(outcome.resolved);
      else if (outcome.status === "own") review.own.push(line.id);
      else if (outcome.status === "stale") review.stale.push(line.id);
      else if (outcome.status === "unsupported") review.unsupported.push(line.id);
      else if (outcome.status === "no-price") review.noPrice.push(line.id);
      else review.unavailable.push(line.id);
    } catch {
      review.unavailable.push(line.id);
    }
  }
  return review;
}

/** A reservation paired with the cart line it pays for, where it was created. */
export type Reservation = { salt: string; lineId: string };

/**
 * After a failure: release what did NOT go out (`spent` is broadcast-and-not-reverted, the pessimistic
 * set), and name what the buyer now owns (`settled`, mined with status 1). Two questions, two inputs.
 */
export function partitionReservations(opts: { reservations: readonly Reservation[]; spent: ReadonlySet<string>; settled: ReadonlySet<string> }): {
  toRelease: string[];
  boughtLineIds: string[];
} {
  const toRelease = [...new Set(opts.reservations.filter((r) => !opts.spent.has(r.salt)).map((r) => r.salt))];
  const boughtLineIds = [...new Set(opts.reservations.filter((r) => opts.settled.has(r.salt)).map((r) => r.lineId))];
  return { toRelease, boughtLineIds };
}

/** The unit price a resolved credits line is debited at: its whole-credit charge. */
export function unitCents(resolved: ResolvedLine): UsdCents | null {
  return resolved.quote.method === "credits" ? resolved.quote.quoted : null;
}
