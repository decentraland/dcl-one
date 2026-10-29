// from: shop@94584a5c:app/src/lib/errors.ts
// One safe mapping from a thrown error to what the buyer reads. Raw backend or wallet text is never
// shown: every path returns curated copy or the caller's own fallback. Wallet-state failures (wrong
// network, an unauthorized wallet) are named on every screen, because "try again" is false for them.

import { isShopApiError } from "../../api/client";
import { chainLabel, isWalletUnauthorizedError, isWrongNetworkError } from "./network";
import { CURRENCY } from "./currency";

type ErrLike = { code?: unknown; status?: number; message?: string; name?: string };

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
