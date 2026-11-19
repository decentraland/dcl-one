// from: shop@94584a5c:app/src/lib/errors.ts
// One safe mapping from a thrown error to what the buyer reads. Raw backend or wallet text is never
// shown: every path returns curated copy or the caller's own fallback. Wallet-state failures (wrong
// network, an unauthorized wallet) are named on every screen, because "try again" is false for them.

import { isShopApiError } from "@shop/api/client";
import { chainLabel, isWalletUnauthorizedError, isWrongNetworkError } from "./network";
import { CURRENCY } from "@shop/checkout/lib/currency";

type ErrLike = { code?: unknown; status?: number; message?: string; name?: string };

/** A relay refusal's own word, as both relay clients keep it (GaslessUnavailableError, MetaTransactionError). */
type Refused = { refusal?: { error?: unknown; retryAt?: unknown; quota?: unknown } };

/** The relay's window: a quota slot frees up at most a day after the transaction that took it. */
const QUOTA_WINDOW_MS = 24 * 60 * 60 * 1000;

/**
 * When a spent quota frees its next slot, as the buyer reads it: "You can buy fee-free again at HH:MM" (or "tomorrow
 * at HH:MM"), the minute rounded up so the time shown is never too early; else "Try again later." The bounds are
 * checked on the number before any date is made: a time already past, or past the window, says nothing reliable.
 */
function quotaWhen(retryAt: unknown, now: Date): string {
  const later = "Try again later.";
  if (typeof retryAt !== "number" || !Number.isSafeInteger(retryAt)) return later;
  const ms = retryAt * 1000;
  if (!(ms > now.getTime() && ms - now.getTime() <= QUOTA_WINDOW_MS + 60_000)) return later;
  const at = new Date(Math.ceil(ms / 60_000) * 60_000);
  try {
    const time = new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit" }).format(at);
    return at.toDateString() === now.toDateString() ? `You can buy fee-free again at ${time}.` : `You can buy fee-free again tomorrow at ${time}.`;
  } catch {
    return later;
  }
}

/**
 * The relay's own limits, as the buyer reads them: the account's fee-free quota over its rolling 24 hours is spent
 * (and when it frees up), or the relay's shared limit on fee-free operations that aren't sales (approvals, ...) is.
 * Null for any other refusal. Never throws: it runs while a failed purchase is still being wound down.
 */
export function relayLimitText(e: unknown, now: Date = new Date()): string | null {
  const refusal = e && typeof e === "object" ? (e as Refused).refusal : undefined;
  if (refusal?.error === "quota_reached") {
    if (refusal.quota === 0) return "Fee-free transactions aren't available for this account.";
    const used = typeof refusal.quota === "number" ? `all ${refusal.quota} fee-free transactions` : "all the fee-free transactions";
    return `You've used ${used} this account gets in 24 hours. ${quotaWhen(refusal.retryAt, now)}`;
  }
  if (refusal?.error === "relay_budget_spent") return "The shared fee-free limit for this operation has been reached. Try again later.";
  return null;
}

/** The buyer bailed out: a wallet rejection (4001), an aborted fetch, or a reject/deny/cancel message. */
export function isRejection(e: unknown): boolean {
  const err = (e ?? {}) as ErrLike;
  return err.code === 4001 || err.code === "ACTION_REJECTED" || err.name === "AbortError" || /reject|denied|cancel/i.test(err.message ?? "");
}

/** Not enough credits (a 402 or an "insufficient" refusal): a top-up prompt, not an error screen. */
export function isInsufficient(e: unknown): boolean {
  if (isShopApiError(e) && e.status === 402) return true;
  const err = (e ?? {}) as ErrLike;
  return err.code === 402 || err.status === 402 || (err.message ?? "").toLowerCase().includes("insufficient");
}

export const ERROR_COPY = {
  rejected: "You cancelled the request in your wallet.",
  walletUnauthorized: "Your wallet did not allow this request. Unlock it, reconnect this site and try again.",
  insufficient: `You don't have enough ${CURRENCY.name} for this purchase.`,
  soldOrRemoved: "This listing was sold or removed.",
  cantBuyOwn: "You can't buy your own listing.",
} as const;

export function wrongNetworkText(current: number, required: number): string {
  return `Your wallet is on ${chainLabel(current)}, but this runs on ${chainLabel(required)}. Switch networks in your wallet and try again.`;
}

/**
 * Map a thrown error to a safe string. Purchase flows pass `sale: true` to also map funds and
 * availability failures; anything unrecognised returns `fallback`.
 */
export function friendlyError(e: unknown, fallback: string, opts: { sale?: boolean } = {}): string {
  if (isRejection(e)) return ERROR_COPY.rejected;
  const limit = relayLimitText(e);
  if (limit) return limit;
  if (isWrongNetworkError(e)) return wrongNetworkText(e.current, e.required);
  if (isWalletUnauthorizedError(e)) return ERROR_COPY.walletUnauthorized;
  if (opts.sale) {
    if (isInsufficient(e)) return ERROR_COPY.insufficient;
    const msg = ((e as ErrLike | null)?.message ?? "").toLowerCase();
    if (/not for sale|not found|no active listing|404/.test(msg) || (isShopApiError(e) && e.status === 404)) return ERROR_COPY.soldOrRemoved;
    if (msg.includes("your own listing")) return ERROR_COPY.cantBuyOwn;
  }
  return fallback;
}
