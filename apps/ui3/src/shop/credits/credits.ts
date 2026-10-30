// from: shop@94584a5c:app/src/lib/credits.ts
// The buyer's credits account over the ONE Shop client (api/shop.ts): balance and holds, the credit
// authorizations a settlement group spends, their release and submission reporting, purchase and
// credit-order histories, and pack order resume/cancel. Upstream signed each call with
// decentraland-crypto-fetch against credits-server; the client signs over the canonical /shop-api path
// (or the legacy alias in legacy mode). Money arrives as integer cents and leaves as whole credits:
// a balance rounds down (model/money creditsFloor), never up.

import { isShopApiError, type ShopApiError } from "../api/client";
import { shopClient, type ShopClient } from "../api/shop";
import type { CreditAuthorization, CreditBatchAuthorization, CreditLineRequest } from "../api/types";
import { creditsFloor, parseUsdCents, ZERO_CENTS, type UsdCents } from "../model/money";

/** A season/program credit as credits-server lists it (legacy MANA-denominated credits). */
export type ServerCredit = {
  id: string;
  userAddress: string;
  amount: string;
  availableAmount: string;
  status: string;
  contract: string;
  timestamp: number;
  signature: string;
  seasonId: number | null;
  expiresAt: number;
  creditSource?: string;
};

/**
 * Credits committed to a purchase not yet proven to have happened; they come back on their own.
 * `balanceCents` has ALREADY subtracted them. `releasesAtSeconds` null means no honest estimate
 * (waiting on chain processing), never zero and never a countdown that ran out.
 */
export type HeldCredits = {
  cents: UsdCents;
  credits: number;
  releasesAtSeconds: number | null;
  purchases: { credits: number; releasesAtSeconds: number | null; contractAddress: string | null; itemId: string | null }[];
};

export type UsdBalance = { balanceCents: UsdCents; credits: number; held?: HeldCredits };

export type UserCreditsResponse = {
  credits: ServerCredit[];
  totalCredits: number;
  totals: { expiring: number; nonExpiring: number };
  usd?: UsdBalance;
};

type Json = Record<string, unknown>;

function obj(v: unknown): Json | null {
  return v && typeof v === "object" && !Array.isArray(v) ? (v as Json) : null;
}

function num(v: unknown, fallback = 0): number {
  const n = typeof v === "string" ? Number(v) : v;
  return typeof n === "number" && Number.isFinite(n) ? n : fallback;
}

function cents(v: unknown): UsdCents {
  const n = typeof v === "string" && /^-?\d+$/.test(v) ? Number(v) : v;
  return parseUsdCents(n) ?? ZERO_CENTS;
}

function decodeHeld(v: unknown): HeldCredits | undefined {
  const h = obj(v);
  if (!h) return undefined;
  const heldCents = cents(h.cents);
  const credits = h.credits === undefined ? creditsFloor(heldCents) : num(h.credits);
  if (credits <= 0 && heldCents <= 0) return undefined;
  const releases = h.releasesAtSeconds;
  return {
    cents: heldCents,
    credits,
    releasesAtSeconds: typeof releases === "number" && Number.isFinite(releases) ? releases : null,
    purchases: (Array.isArray(h.purchases) ? h.purchases : []).map((p) => {
      const o = obj(p) ?? {};
      return {
        credits: num(o.credits),
        releasesAtSeconds: typeof o.releasesAtSeconds === "number" ? o.releasesAtSeconds : null,
        contractAddress: typeof o.contractAddress === "string" ? o.contractAddress : null,
        itemId: typeof o.itemId === "string" || typeof o.itemId === "number" ? String(o.itemId) : null,
      };
    }),
  };
}

/** The `usd` block: whole credits are the balance's floor; `held` only when something is held. */
export function decodeUsdBalance(v: unknown): UsdBalance {
  const u = obj(v);
  if (!u) return { balanceCents: ZERO_CENTS, credits: 0 };
  const balanceCents = cents(u.balanceCents);
  const held = decodeHeld(u.held);
  return { balanceCents, credits: creditsFloor(balanceCents), ...(held ? { held } : {}) };
}

export function decodeUserCredits(v: unknown): UserCreditsResponse {
  const r = obj(v) ?? {};
  const totals = obj(r.totals) ?? {};
  return {
    credits: (Array.isArray(r.credits) ? r.credits : []) as ServerCredit[],
    totalCredits: num(r.totalCredits),
    totals: { expiring: num(totals.expiring), nonExpiring: num(totals.nonExpiring) },
    ...(r.usd !== undefined ? { usd: decodeUsdBalance(r.usd) } : {}),
  };
}

/** GET /shop-api/v1/users/{address}/credits (signed: the caller must be `address`). */
export async function getUserCredits(address: string, client: ShopClient = shopClient(), signal?: AbortSignal): Promise<UserCreditsResponse> {
  return decodeUserCredits(await client.credits.balance<unknown>(address, { signal }));
}

/** The spendable USD balance; zero when the feature is off or nothing was bought yet. */
export async function getUsdBalance(address: string, client: ShopClient = shopClient(), signal?: AbortSignal): Promise<UsdBalance> {
  const { usd } = await getUserCredits(address, client, signal);
  return usd ?? { balanceCents: ZERO_CENTS, credits: 0 };
}

/** One checkout line: what is bought and what it is quoted at. The credit that pays for it is shared. */
export type CheckoutLine = CreditLineRequest;

/**
 * ONE credit for ONE settlement group (POST /shop-api/v1/credit-authorizations/batch). useCredits
 * consumes a list of credits in order until the call is covered, so per-line credits leave an
 * unreachable tail; one credit per transaction has none. Call once per group, never per item.
 */
export async function authorizeUsdCreditGroup(
  lines: CheckoutLine[],
  source: "website" | "client" = "website",
  client: ShopClient = shopClient(),
): Promise<CreditBatchAuthorization> {
  return client.authorizations.authorizeGroup({ items: lines, source });
}

/** One line's credit (POST /shop-api/v1/credit-authorizations): single purchases and NAME registrations. */
export async function authorizeUsdCredit(
  usdPriceCents: number,
  opts: { tradeId?: string; item?: { contractAddress: string; itemId: string }; registeredName?: string; source?: "website" | "client" } = {},
  client: ShopClient = shopClient(),
): Promise<CreditAuthorization> {
  return client.authorizations.authorize({
    usdPriceCents,
    ...(opts.tradeId ? { tradeId: opts.tradeId } : {}),
    ...(opts.item ?? {}),
    ...(opts.registeredName ? { name: opts.registeredName } : {}),
    source: opts.source ?? "website",
  });
}

/** One row of the purchase history: SETTLED on chain, PENDING reserved, EXPIRED released. */
export type PurchaseRecord = {
  id: string;
  tradeId: string | null;
  contractAddress: string | null;
  itemId: string | null;
  registeredName: string | null;
  usdCents: number;
  credits: number;
  status: "PENDING" | "SETTLED" | "EXPIRED";
  createdAt: number;
  manaSettledWei: string | null;
  /** The settlement hash, derived from on-chain consumption (shared by every line of one transaction). */
  txHash: string | null;
  /** The transaction the credit was SUBMITTED in, reported by the client: what tells a revert from an abandoned modal. */
  submittedTxHash: string | null;
};

const PURCHASE_STATUSES = ["PENDING", "SETTLED", "EXPIRED"] as const;

function str(v: unknown): string | null {
  return typeof v === "string" && v ? v : typeof v === "number" ? String(v) : null;
}

export function decodePurchaseRecord(v: unknown): PurchaseRecord | null {
  const p = obj(v);
  if (!p || typeof p.id !== "string") return null;
  const status = PURCHASE_STATUSES.find((s) => s === p.status);
  if (!status) return null;
  const usdCents = num(p.usdCents);
  return {
    id: p.id,
    tradeId: str(p.tradeId),
    contractAddress: str(p.contractAddress),
    itemId: str(p.itemId),
    registeredName: str(p.registeredName),
    usdCents,
    credits: p.credits === undefined ? creditsFloor(cents(usdCents)) : num(p.credits),
    status,
    createdAt: num(p.createdAt),
    manaSettledWei: str(p.manaSettledWei),
    txHash: str(p.txHash) ?? str(p.transactionHash),
    submittedTxHash: str(p.submittedTxHash),
  };
}

function pageTotal(json: Json, skip: number, first: number | undefined, count: number): number {
  if (typeof json.total === "number") return json.total;
  const size = first ?? count;
  return skip + count + (size > 0 && count >= size ? 1 : 0);
}

/** The buyer's purchase history, paginated; `all` includes pending and expired intents. */
export async function fetchUserPurchases(
  address: string,
  opts: { all?: boolean; first?: number; skip?: number } = {},
  client: ShopClient = shopClient(),
): Promise<{ items: PurchaseRecord[]; total: number }> {
  const json =
    obj(
      await client.credits.purchases<unknown>(address, {
        ...(opts.all ? { status: "all" } : {}),
        ...(opts.first != null ? { limit: opts.first } : {}),
        ...(opts.skip != null ? { offset: opts.skip } : {}),
      }),
    ) ?? {};
  const items = (Array.isArray(json.purchases) ? json.purchases : []).map(decodePurchaseRecord).filter((p): p is PurchaseRecord => p !== null);
  return { items, total: pageTotal(json, opts.skip ?? 0, opts.first, items.length) };
}

/**
 * credits-server's own pack-order vocabulary: `initiated` (checkout opened, nobody paid, still
 * payable), `processing`/`crediting` (money in, credits not yet in the balance), `credited`, `failed`,
 * `abandoned` (retired unpaid).
 */
export type CreditOrderStatus = "initiated" | "processing" | "crediting" | "credited" | "failed" | "abandoned";

export type CreditOrder = { id: string; credits: number; usdCents: number; status: CreditOrderStatus; createdAt: number };

/** A seller payout row the credit-orders endpoint may carry; the unified Shop pays no resale proceeds in credits, so it is only read. */
export type Payout = { txHash?: string; credits?: number; status?: string; availableAt?: number };

const ORDER_STATUSES: readonly CreditOrderStatus[] = ["initiated", "processing", "crediting", "credited", "failed", "abandoned"];

export function decodeCreditOrder(v: unknown): CreditOrder | null {
  const o = obj(v);
  const status = ORDER_STATUSES.find((s) => s === o?.status);
  if (!o || typeof o.id !== "string" || !status) return null;
  return { id: o.id, credits: num(o.credits), usdCents: num(o.usdCents), status, createdAt: num(o.createdAt) };
}

/** A credit order's status in the Activity pill's vocabulary. */
export function creditOrderPill(status: CreditOrderStatus): "SETTLED" | "PENDING" | "UNFINISHED" | "FAILED" {
  if (status === "credited") return "SETTLED";
  if (status === "failed" || status === "abandoned") return "FAILED";
  if (status === "initiated") return "UNFINISHED";
  return "PENDING";
}

/** The buyer's pack orders, newest first. Never throws: an unavailable endpoint reads as no orders. */
export async function fetchUserCreditOrders(
  address: string,
  opts: { first?: number; skip?: number } = {},
  client: ShopClient = shopClient(),
): Promise<{ items: CreditOrder[]; total: number; payouts: Payout[] }> {
  try {
    const json =
      obj(
        await client.credits.creditOrders<unknown>(address, {
          ...(opts.first != null ? { limit: opts.first } : {}),
          ...(opts.skip != null ? { offset: opts.skip } : {}),
        }),
      ) ?? {};
    const items = (Array.isArray(json.orders) ? json.orders : []).map(decodeCreditOrder).filter((o): o is CreditOrder => o !== null);
    const earnings = obj(json.earnings);
    const payouts = (Array.isArray(earnings?.items) ? earnings.items : []) as Payout[];
    return { items, total: pageTotal(json, opts.skip ?? 0, opts.first, items.length), payouts };
  } catch {
    return { items: [], total: 0, payouts: [] };
  }
}

/** The buyer walked away from a pack checkout. Best-effort and silent: the server expires it anyway. */
export async function cancelCreditOrder(orderId: string, client: ShopClient = shopClient()): Promise<void> {
  try {
    await client.packs.cancelOrder(orderId);
  } catch {
    // The server retires an unpaid order on its own (checkout.session.expired).
  }
}

/** Four different things to SAY about reopening a checkout, never collapsed into one. */
export type ResumeResult = { kind: "url"; url: string } | { kind: "expired" } | { kind: "paid" } | { kind: "unavailable" };

/** Reopen a pack checkout the buyer left, or say why it cannot be (the 409 body carries the order's status). */
export async function resumeCreditOrder(orderId: string, client: ShopClient = shopClient()): Promise<ResumeResult> {
  try {
    const json = obj(await client.packs.resumeOrder<unknown>(orderId));
    return typeof json?.url === "string" && json.url ? { kind: "url", url: json.url } : { kind: "unavailable" };
  } catch (e) {
    if (!isShopApiError(e) || (e as ShopApiError).status !== 409) return { kind: "unavailable" };
    const status = obj((e as ShopApiError).body)?.status;
    if (status === "processing" || status === "crediting" || status === "credited") return { kind: "paid" };
    return typeof status === "string" && status ? { kind: "expired" } : { kind: "unavailable" };
  }
}

/** Release the reservations nothing was handed off for (by salt). No-op for an empty list. */
export async function cancelUsdIntents(salts: string[], client: ShopClient = shopClient()): Promise<number> {
  const unique = [...new Set(salts)];
  if (unique.length === 0) return 0;
  const out = await client.authorizations.cancel({ salts: unique });
  return out?.released ?? 0;
}

/** Record the transaction a group's credits went out in: a FACT about a transaction, never an outcome. */
export async function reportIntentSubmission(salts: string[], txHash: string, client: ShopClient = shopClient()): Promise<number> {
  const unique = [...new Set(salts)];
  if (unique.length === 0) return 0;
  const out = await client.authorizations.submitted({ salts: unique, txHash });
  return out?.recorded ?? 0;
}
