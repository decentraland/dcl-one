// What a failed or unfinished wallet step tells the buyer. A declined request is an answer, not an
// error; a transaction that may still land is pending, never "failed, try again"; a wrong network
// names both networks. None of these ever points to credits.

import { isShopApiError } from "../api/client";
import { chainLabel } from "../model/chain";
import { isUserRejection, isWrongNetworkError, WrongNetworkError } from "../wallet/chain";
import { AccountChangedError, TxPendingError, TxRevertedError } from "./evm";
import { RentalIndexLagError, RentalInputError } from "./rentals";

export type TxOutcome =
  | { kind: "declined"; text: string }
  | { kind: "pending"; text: string; txHash: string }
  | { kind: "indexing"; text: string }
  | { kind: "failed"; text: string; retry: boolean };

export function txOutcome(e: unknown): TxOutcome {
  if (isUserRejection(e)) return { kind: "declined", text: "You declined the request in your wallet. Nothing was sent." };
  if (e instanceof TxPendingError) {
    return { kind: "pending", text: "The transaction was sent and is still confirming. It may take a few minutes; check it on the block explorer before trying again.", txHash: e.txHash };
  }
  if (e instanceof RentalIndexLagError) return { kind: "indexing", text: e.message };
  if (isWrongNetworkError(e)) {
    const w = e as WrongNetworkError;
    return { kind: "failed", text: `Your wallet is on ${chainLabel(w.current)}. Switch it to ${chainLabel(w.required)} to continue.`, retry: true };
  }
  if (e instanceof AccountChangedError) return { kind: "failed", text: e.message + " Switch back, or sign in again with the new account.", retry: true };
  if (e instanceof TxRevertedError) return { kind: "failed", text: "The network reverted the transaction. Nothing changed; check the details and try again.", retry: true };
  if (e instanceof RentalInputError) return { kind: "failed", text: e.message, retry: true };
  if (isShopApiError(e)) return { kind: "failed", text: e.message, retry: e.status !== 401 };
  return { kind: "failed", text: (e as { message?: string } | null)?.message || "Something went wrong. Please try again.", retry: true };
}
