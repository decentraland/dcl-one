// from: shop@94584a5c:app/src/lib/payments.ts
// The "get credits" money path: the pack catalogue (authoritative on the server; the bundled packs are
// only a typed fallback so the no-funds pickers render before or without the fetch), which packs a
// no-funds picker may honestly offer, and starting and awaiting a pack purchase. Credits are fixed USD:
// one credit is $0.10, and the charge (`usd`) is what the card pays while `credits` is what the balance
// gains; they differ on purpose (the Stripe fee). Nothing here exposes Stripe, cards or USDC.

import { shopClient, type ShopClient } from "../api/shop";
import type { PackOrderStatus } from "../api/types";
import { createPackCheckoutReal, pollCreditGrantReal } from "./payments-stripe";

export const USD_PER_CREDIT = 0.1;

export type CreditPack = {
  id: string;
  usd: number;
  credits: number;
  bestValue?: boolean;
  /** The catalogue's artwork (webp first), when it publishes one. */
  artUrl?: string;
};

/** Fallback only: checkout is always priced by the server from the pack id. Mirrors the server catalogue. */
export const CREDIT_PACKS: CreditPack[] = [
  { id: "pack_5", usd: 5.99, credits: 40 },
  { id: "pack_10", usd: 11.99, credits: 100, bestValue: true },
  { id: "pack_25", usd: 29.99, credits: 260 },
  { id: "pack_50", usd: 59.99, credits: 540 },
];

/** How many packs a no-funds picker offers in one row. */
export const MAX_OFFER_PACKS = 4;

/**
 * The bonus a pack carries over the ENTRY pack's rate (the only baseline the buyer can transact at);
 * null for the entry pack, a single-pack catalogue, or a pack that does not beat the entry rate.
 */
export function packBonus(pack: CreditPack, packs: CreditPack[]): { baseline: number; bonus: number } | null {
  const entry = packs.reduce<CreditPack | undefined>((cheapest, p) => (!cheapest || p.usd < cheapest.usd ? p : cheapest), undefined);
  if (!entry || entry.id === pack.id || !(entry.usd > 0)) return null;
  const baseline = Math.floor((entry.credits / entry.usd) * pack.usd);
  const bonus = pack.credits - baseline;
  return bonus > 0 ? { baseline, bonus } : null;
}

/**
 * Which packs a no-funds picker may offer. Every pack there promises that buying it FINISHES the
 * purchase, so one smaller than the shortfall is filtered out; when nothing covers it the whole list is
 * offered and `closesGap` says the recommendation is progress, not the answer.
 */
export function offerablePacks(packs: CreditPack[], shortfallCredits: number): { packs: CreditPack[]; recommended: CreditPack | null; closesGap: boolean } {
  const shortfall = Math.max(0, shortfallCredits);
  const covering = shortfall > 0 ? packs.filter((p) => p.credits >= shortfall) : packs;
  const closesGap = shortfall <= 0 || covering.length > 0;
  const offered = covering.length > 0 ? covering : packs;
  if (offered.length === 0) return { packs: offered, recommended: null, closesGap };
  const recommended = closesGap
    ? offered.reduce((best, p) => (p.credits < best.credits ? p : best))
    : offered.reduce((best, p) => (p.credits > best.credits ? p : best));
  return { packs: offered, recommended, closesGap };
}

type ServerCreditPack = { id: string; usd: number; credits: number; recommended?: boolean; order?: number; imageUrl?: string; imageUrlWebp?: string };

/** GET /shop-api/v1/packs, mapped onto CreditPack and ordered. */
export async function fetchCreditPacks(client: ShopClient = shopClient(), signal?: AbortSignal): Promise<CreditPack[]> {
  const body = (await client.packs.list<{ packs?: ServerCreditPack[] }>({ signal })) ?? {};
  const packs = Array.isArray(body.packs) ? body.packs : [];
  return packs
    .filter((p) => p && typeof p.id === "string" && Number.isFinite(p.usd) && Number.isFinite(p.credits))
    .slice()
    .sort((a, b) => (a.order ?? 0) - (b.order ?? 0))
    .map((p) => {
      const art = p.imageUrlWebp || p.imageUrl || undefined;
      return { id: p.id, usd: p.usd, credits: p.credits, ...(p.recommended ? { bestValue: true } : {}), ...(art ? { artUrl: art } : {}) };
    });
}

/** Credits for a USD amount at the spend peg (not the buy rate). */
export function creditsForUsd(usd: number): number {
  return Math.round(usd / USD_PER_CREDIT);
}

/** USD for an amount of credits at the spend peg. */
export function usdForCredits(credits: number): number {
  return Math.round(credits * USD_PER_CREDIT * 100) / 100;
}

export function getPack(packId: string, packs: CreditPack[] = CREDIT_PACKS): CreditPack | undefined {
  return packs.find((p) => p.id === packId);
}

export type CheckoutSession = {
  orderId: string;
  /** Stripe's hosted page to send the browser to; absent when the server settles without leaving (mock). */
  url?: string;
  mock: boolean;
};

/** The server's order vocabulary plus 'pending': the poll gave up with money in and the grant not written. */
export type OrderStatus = { status: PackOrderStatus["status"] | "initiated" | "crediting" | "abandoned" | "pending"; creditsGranted?: number; newBalance?: number; error?: string };

export const MOCK_ORDER_PREFIX = "mock_cs_";

type PaymentsWindow = Window & typeof globalThis & { __SHOP_PAYMENTS__?: string };

/**
 * Mock payments exist only for stories and tests (`window.__SHOP_PAYMENTS__ = "mock"`). Unlike upstream,
 * a missing Stripe key never falls back to a mock: the node decides, and its answer is the order.
 */
export function isMockPayments(): boolean {
  return typeof window !== "undefined" && (window as PaymentsWindow).__SHOP_PAYMENTS__ === "mock";
}

/** Start a pack purchase: a Stripe hosted Checkout (real) or a local order that grants after a moment (mock). */
export async function createPackCheckout(packId: string, client: ShopClient = shopClient()): Promise<CheckoutSession> {
  if (!packId) throw new Error("Unknown pack");
  if (isMockPayments()) return { orderId: `${MOCK_ORDER_PREFIX}${packId}_${Date.now()}`, mock: true };
  return createPackCheckoutReal(packId, client);
}

/** Wait for the verified webhook to grant the pack's credits (real) or simulate the grant (mock). */
export async function pollCreditGrant(
  orderId: string,
  opts: { intervalMs?: number; timeoutMs?: number; signal?: AbortSignal; packs?: CreditPack[] } = {},
  client: ShopClient = shopClient(),
): Promise<OrderStatus> {
  if (orderId.startsWith(MOCK_ORDER_PREFIX)) {
    await new Promise((resolve) => setTimeout(resolve, opts.intervalMs ?? 300));
    const packId = orderId.slice(MOCK_ORDER_PREFIX.length).replace(/_\d+$/, "");
    const pack = getPack(packId, opts.packs ?? CREDIT_PACKS);
    return pack ? { status: "credited", creditsGranted: pack.credits } : { status: "failed", error: "Unknown pack" };
  }
  return pollCreditGrantReal(orderId, opts, client);
}
