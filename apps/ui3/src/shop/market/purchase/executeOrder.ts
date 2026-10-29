// from: marketplace@f3956826:webapp/src/modules/order/sagas.ts (EXECUTE_ORDER), webapp/src/modules/order/utils.ts
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.
//
// Buying a listed token (a resale, LAND, an estate or a NAME resale) is MANA only. Upstream let this saga
// pay with credits (useCreditsMarketplace / useCreditsLegacyMarketplace) and refused only estates; here
// there is no credits parameter at all, so no caller can route a resale through the CreditsManager (the
// inventory's forbidden credit path for EXECUTE_ORDER). The buyer allows the marketplace the MANA, then
// accepts the signed trade, or executes the legacy on-chain order.

import type { Eip1193Provider } from "../../../data/auth/wallet";
import type { ChainId } from "../../model/chain";
import { withCapHeadroom, type ManaWei } from "../../model/money";
import type { Order } from "../lib/api";
import { LEGACY_MARKETPLACE, LEGACY_MARKETPLACE_ABI, marketplaceForTrade } from "../lib/contracts";
import { ensureManaAllowance } from "../lib/mana";
import { acceptTrade, TRADE_ASSET, type Trade } from "../lib/trades";
import { sendCall, waitForReceipt, type WaitOptions } from "../lib/tx";

export type OrderPhase = "approving" | "confirming" | "pending";

/**
 * How much MANA the marketplace may take for a trade: the MANA leg exactly, or for a USD-pegged order the
 * server's current MANA quote with the upstream cap headroom (the contract converts at the oracle rate at
 * settlement, so the allowance must not fall short of a small move).
 */
export function manaAllowanceFor(trade: Trade, quote: ManaWei | null, headroomPct = 105): bigint {
  const leg = trade.received.find((a) => a.assetType === TRADE_ASSET.ERC20 || a.assetType === TRADE_ASSET.USD_PEGGED_MANA);
  if (!leg || !("amount" in leg)) throw new Error("This listing has no MANA price.");
  if (leg.assetType === TRADE_ASSET.ERC20) return BigInt(leg.amount);
  if (quote === null) throw new Error("The MANA price is unavailable right now. Try again in a moment.");
  const capped = withCapHeadroom(quote, headroomPct);
  if (capped === null) throw new Error("The MANA price is out of range.");
  return capped;
}

/** EXECUTE_ORDER for a signed trade listing: MANA allowance, then `accept([trade])`. */
export async function buyTradeWithMana(opts: {
  provider: Eip1193Provider;
  trade: Trade;
  /** The server's current MANA quote for a USD-pegged listing (a PurchaseOffer's MANA quote). */
  manaQuote?: ManaWei | null;
  onPhase?: (phase: OrderPhase) => void;
  onSubmitted?: (hash: string) => void;
  wait?: WaitOptions;
}): Promise<string> {
  const marketplace = marketplaceForTrade({ contract: opts.trade.contract, chainId: opts.trade.chainId });
  if (!marketplace) throw new Error("This listing names a contract that is not a marketplace.");
  opts.onPhase?.("approving");
  await ensureManaAllowance({
    provider: opts.provider,
    chainId: opts.trade.chainId,
    spender: marketplace.address,
    amount: manaAllowanceFor(opts.trade, opts.manaQuote ?? null),
    wait: opts.wait,
  });
  opts.onPhase?.("confirming");
  return acceptTrade({
    provider: opts.provider,
    trade: opts.trade,
    wait: opts.wait,
    onSubmitted: (hash) => {
      opts.onPhase?.("pending");
      opts.onSubmitted?.(hash);
    },
  });
}

/**
 * EXECUTE_ORDER for a legacy on-chain order: MANA allowance to the legacy marketplace, then `executeOrder`,
 * or, for an Estate, `safeExecuteOrder` bound to the composition fingerprint the buyer reviewed (upstream
 * OrderService.execute): the registry rejects the sale if the Estate changed since. The venue must be the
 * chain's legacy marketplace: an order naming any other contract is refused before an allowance is asked.
 */
export async function executeLegacyOrder(opts: {
  provider: Eip1193Provider;
  order: Pick<Order, "contractAddress" | "tokenId" | "price" | "chainId" | "marketplaceAddress">;
  /** An Estate's registry fingerprint, read when the buyer reviewed it. */
  fingerprint?: `0x${string}`;
  onPhase?: (phase: OrderPhase) => void;
  onSubmitted?: (hash: string) => void;
  wait?: WaitOptions;
}): Promise<string> {
  const chainId = opts.order.chainId as ChainId;
  const legacy = LEGACY_MARKETPLACE[chainId];
  const named = opts.order.marketplaceAddress?.toLowerCase();
  if (!legacy || (named && named !== legacy.address.toLowerCase()) || !opts.order.tokenId) throw new Error("This order cannot be executed from the Shop.");
  const venue = legacy.address;
  const price = BigInt(opts.order.price);
  opts.onPhase?.("approving");
  await ensureManaAllowance({ provider: opts.provider, chainId, spender: venue, amount: price, wait: opts.wait });
  opts.onPhase?.("confirming");
  const assetId = BigInt(opts.order.tokenId);
  const hash = await sendCall(
    opts.provider,
    opts.fingerprint
      ? { chainId, to: venue, abi: LEGACY_MARKETPLACE_ABI, functionName: "safeExecuteOrder", args: [opts.order.contractAddress, assetId, price, opts.fingerprint] }
      : { chainId, to: venue, abi: LEGACY_MARKETPLACE_ABI, functionName: "executeOrder", args: [opts.order.contractAddress, assetId, price] },
  );
  opts.onPhase?.("pending");
  opts.onSubmitted?.(hash);
  return waitForReceipt(chainId, hash, "The purchase", opts.wait);
}

/** marketplace CANCEL_ORDER for a legacy on-chain order (its seller only). */
export async function cancelLegacyOrder(opts: {
  provider: Eip1193Provider;
  order: Pick<Order, "contractAddress" | "tokenId" | "chainId" | "marketplaceAddress">;
  wait?: WaitOptions;
}): Promise<string> {
  const chainId = opts.order.chainId as ChainId;
  const venue = opts.order.marketplaceAddress?.toLowerCase() || LEGACY_MARKETPLACE[chainId]?.address;
  if (!venue || !opts.order.tokenId) throw new Error("This order cannot be cancelled from the Shop.");
  const hash = await sendCall(opts.provider, {
    chainId,
    to: venue,
    abi: LEGACY_MARKETPLACE_ABI,
    functionName: "cancelOrder",
    args: [opts.order.contractAddress, BigInt(opts.order.tokenId)],
  });
  return waitForReceipt(chainId, hash, "The cancellation", opts.wait);
}
