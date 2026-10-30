// from: marketplace@f3956826:webapp/src/modules/bid/sagas.ts, webapp/src/modules/bid/utils.ts,
//       webapp/src/modules/bid/actions.ts (PLACE_BID, ACCEPT_BID, CANCEL_BID, ARCHIVE_BID, UNARCHIVE_BID)
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.
//
// Bids are MANA only (SHOP.md: every secondary sale, and an offer on a token is one). A bid is a signed
// off-chain trade the owner accepts on the marketplace; a legacy bid (no trade id) lives on the old Bid
// contract, accepted by sending the token to it and cancelled there. No credit, top-up or credits+MANA
// option exists anywhere in this module.

import type { Eip1193Provider } from "../../../data/auth/wallet";
import type { ChainId } from "../../model/chain";
import { parseManaWei } from "../../model/money";
import { LEGACY_ACCEPT_BID_ABI, LEGACY_BID, LEGACY_BID_ABI, latestOffChainMarketplace } from "../lib/contracts";
import type { Bid } from "../lib/api";
import { ensureManaAllowance } from "../lib/mana";
import { acceptTrade, cancelTrade, createBidTrade, ensureApproval, fetchTrade, postTrade, type Trade } from "../lib/trades";
import { sendCall, waitForReceipt, walletAccount, type WaitOptions } from "../lib/tx";

export type BidTarget = { contractAddress: string; chainId: ChainId } & ({ tokenId: string } | { itemId: string });

/** The progress a bid placement reports, so the page can say which wallet prompt is which. */
export type BidPhase = "approving" | "signing" | "publishing";

export const DEFAULT_BID_DAYS = 30;

/** marketplace `isBidTrade`: an off-chain bid carries its trade id. */
export function isBidTrade(bid: Bid): boolean {
  return typeof bid.tradeId === "string" && bid.tradeId.length > 0;
}

/**
 * PLACE_BID: allow the marketplace to take the bid's MANA when it is accepted, sign the bid trade, post it.
 * Returns the stored trade.
 */
export async function placeBid(opts: {
  provider: Eip1193Provider;
  target: BidTarget;
  priceWei: bigint;
  expiresAtMs: number;
  fingerprint?: string;
  onPhase?: (phase: BidPhase) => void;
  wait?: WaitOptions;
}): Promise<Trade> {
  const marketplace = latestOffChainMarketplace(opts.target.chainId);
  opts.onPhase?.("approving");
  await ensureManaAllowance({ provider: opts.provider, chainId: opts.target.chainId, spender: marketplace.address, amount: opts.priceWei, wait: opts.wait });
  opts.onPhase?.("signing");
  const trade = await createBidTrade({
    provider: opts.provider,
    asset: opts.target,
    priceWei: opts.priceWei,
    expiresAtMs: opts.expiresAtMs,
    fingerprint: opts.fingerprint,
  });
  opts.onPhase?.("publishing");
  return postTrade(trade);
}

/**
 * ACCEPT_BID: the owner sends the token and receives the bid's MANA. An off-chain bid needs the marketplace
 * approved for the collection first (once); a legacy bid is accepted by transferring the token to the Bid
 * contract with the bid id as data.
 */
export async function acceptBid(opts: {
  provider: Eip1193Provider;
  bid: Bid;
  onSubmitted?: (hash: string) => void;
  wait?: WaitOptions;
}): Promise<string> {
  const { bid } = opts;
  if (isBidTrade(bid)) {
    const trade = await fetchTrade(bid.tradeId!);
    await ensureApproval({ provider: opts.provider, contractAddress: bid.contractAddress, chainId: bid.chainId, wait: opts.wait });
    return acceptTrade({ provider: opts.provider, trade, onSubmitted: opts.onSubmitted, wait: opts.wait });
  }
  const owner = await walletAccount(opts.provider);
  const bidContract = bid.bidAddress ?? LEGACY_BID[bid.chainId]?.address;
  if (!bidContract || !bid.tokenId || !bid.blockchainId) throw new Error("This bid cannot be accepted from the Shop: it names no Bid contract or bid id.");
  const hash = await sendCall(opts.provider, {
    chainId: bid.chainId,
    to: bid.contractAddress,
    abi: LEGACY_ACCEPT_BID_ABI,
    functionName: "safeTransferFrom",
    args: [owner, bidContract, BigInt(bid.tokenId), bid.blockchainId],
  });
  opts.onSubmitted?.(hash);
  return waitForReceipt(bid.chainId, hash, "Accepting the bid", opts.wait);
}

/** CANCEL_BID: only the bidder cancels; an off-chain bid's signature is invalidated on the marketplace. */
export async function cancelBid(opts: { provider: Eip1193Provider; bid: Bid; wait?: WaitOptions }): Promise<string> {
  const { bid } = opts;
  const bidder = await walletAccount(opts.provider);
  if (bidder !== bid.bidder.toLowerCase()) throw new Error("Only the account that placed this bid can cancel it.");
  if (isBidTrade(bid)) return cancelTrade({ provider: opts.provider, trade: await fetchTrade(bid.tradeId!), wait: opts.wait });
  const bidContract = bid.bidAddress ?? LEGACY_BID[bid.chainId]?.address;
  if (!bidContract || !bid.tokenId) throw new Error("This bid cannot be cancelled from the Shop: it names no Bid contract.");
  const hash = await sendCall(opts.provider, {
    chainId: bid.chainId,
    to: bidContract,
    abi: LEGACY_BID_ABI,
    functionName: "cancelBid",
    args: [bid.contractAddress, BigInt(bid.tokenId)],
  });
  return waitForReceipt(bid.chainId, hash, "Cancelling the bid", opts.wait);
}

/** marketplace `checkFingerprint`: an estate bid is only valid against the fingerprint it named. */
export function checkFingerprint(bid: Pick<Bid, "fingerprint">, fingerprint: string | undefined): boolean {
  if (fingerprint && bid.fingerprint) return fingerprint === bid.fingerprint;
  return true;
}

/** The bid's MANA, exactly; null for a malformed row. */
export function bidPrice(bid: Pick<Bid, "price">) {
  return parseManaWei(bid.price);
}

/** Whether a bid has expired (`expiresAt` is milliseconds on the wire). */
export function isBidExpired(bid: Pick<Bid, "expiresAt">, now = Date.now()): boolean {
  return bid.expiresAt > 0 && bid.expiresAt <= now;
}
