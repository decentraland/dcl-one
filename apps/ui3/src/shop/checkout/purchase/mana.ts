// from: shop@94584a5c:app/src/lib/buy-mana.ts
// The MANA rails. MANA alone: marketplace.accept (or acceptWithCoupon) for listings and
// CollectionStore.buy for mints, paid from the buyer's MANA, relayed when possible and otherwise sent
// by the wallet (which must already be on the chain). Credits + MANA (only inside the MANA option of an
// eligible primary line): useCredits with a partial credit and the remainder pulled from the buyer's
// MANA by the CreditsManager. The allowance is checked first and sized to THIS purchase: a leftover
// smaller one would revert the settlement. The CreditsManager is never left an unlimited allowance:
// it is approved exactly the gap, and only when what it already has falls short of it.

import type { Hex } from "viem";

import type { ChainId } from "@shop/model/chain";
import { gaslessEnabled } from "../chain/config";
import { requireContracts } from "../chain/contracts";
import { GaslessUnavailableError, isGaslessUnavailable, isSignatureDeclined, provedNotSent, relayMetaTransaction, type MetaTxTarget } from "../chain/metatx";
import { isRevertedTxError, isWalletRefusal, revertedTxHash, sendAndConfirm, TxPendingError } from "../chain/wallet";
import { AuthorizationKind, ensureAuthorization } from "../lib/authorizations";
import { getMarketplaceForTrade } from "../lib/marketplace";
import { canPayGasItself } from "../lib/wallet-kind";
import { buildGroupUseCreditsArgs, buyOneWithCredits, creditsManagerFor, groupPurchases, type AnyPurchase } from "./credits";
import { encodeMarketplaceCall, encodeStoreBuy, type ListingCoupon, type SignedTrade, type SpendableCredit, type StoreItemToBuy } from "./encoding";
import { buyOneGasless, isSettlementPending, waitForSettlement } from "./gasless";
import type { RailContext } from "./rail";
import { confirmMetaTx, isMetaTxPending } from "./tx-confirm";
import { isRequestLimited } from "../lib/errors";

export type MintToBuy = { item: StoreItemToBuy; chainId: ChainId };

export type PurchaseTarget = { kind: "trade"; trade: SignedTrade; coupon?: ListingCoupon } | { kind: "store"; mint: MintToBuy };

export function targetChainId(target: PurchaseTarget): ChainId {
  return target.kind === "trade" ? target.trade.chainId : target.mint.chainId;
}

/** The credits-rail purchase for a target (trade or mint). */
export function purchaseFor(target: PurchaseTarget, credits: SpendableCredit[], maxCreditedValue: string): AnyPurchase {
  return target.kind === "trade"
    ? { kind: "trade", trade: target.trade, coupon: target.coupon, credits, maxCreditedValue }
    : { kind: "store", item: target.mint.item, chainId: target.mint.chainId, credits, maxCreditedValue };
}

/** Who pulls the buyer's MANA: the marketplace or the store (MANA rail), the CreditsManager (combined). */
export function manaSpenderFor(rail: "mana" | "combined", target: PurchaseTarget): { spender: string; chainId: ChainId } {
  const chainId = targetChainId(target);
  const book = requireContracts(chainId);
  if (rail === "combined") {
    if (!book.creditsManager) throw new Error(`No CreditsManager on chain ${chainId}`);
    return { spender: book.creditsManager.address, chainId };
  }
  if (target.kind === "trade") {
    const market = getMarketplaceForTrade(target.trade);
    if (!market) throw new Error("The listing names no marketplace deployed on its chain");
    return { spender: market.address, chainId };
  }
  if (!book.collectionStore) throw new Error(`No CollectionStore on chain ${chainId}`);
  return { spender: book.collectionStore.address, chainId };
}

/** `exact`: an allowance short of `requiredWei` is set to exactly `requiredWei` instead of in full. */
async function ensureManaAllowance(ctx: RailContext, chainId: ChainId, spender: string, requiredWei: bigint | undefined, exact = false): Promise<void> {
  await ensureAuthorization({
    auth: { kind: AuthorizationKind.Allowance, contractAddress: requireContracts(chainId).mana.address, spenderAddress: spender, chainId },
    provider: ctx.provider,
    account: ctx.buyer,
    walletKind: ctx.walletKind,
    reader: ctx.reader,
    relayerUrl: ctx.relayerUrl,
    fetchImpl: ctx.fetchImpl,
    locks: ctx.locks,
    grantLockHeld: ctx.grantLockHeld,
    live: ctx.live,
    requiredWei,
    exact,
  });
}

/**
 * Relay `data` to `target` when gasless is on; a declined prompt or a relayed-but-unconfirmed call
 * propagates (never re-submitted directly: that could run it twice). Otherwise the wallet sends it.
 * `onSigned` marks the hand-off: after the relayed signature (right before the relay is called), or
 * right before the wallet is asked to send. From then on the call may be broadcast. With `ctx.relayOnly`, a call the
 * relay can't run (or the relay off) is never sent from the wallet instead.
 */
async function relayOrSend(
  ctx: RailContext,
  opts: { target: MetaTxTarget; data: Hex; onSigned?: () => void; onBroadcast?: (txHash: Hex) => void; what: string },
): Promise<Hex> {
  const gasless = ctx.gasless ?? gaslessEnabled();
  // Relay only (ctx.relayOnly): with the relay off, or refusing, the purchase isn't sent at all.
  if (ctx.relayOnly && !gasless) throw new GaslessUnavailableError("Fee-free checkout is off, so this purchase wasn't sent.", "relayer-rejected");
  if (gasless) {
    try {
      const { txHash } = await relayMetaTransaction({
        provider: ctx.provider,
        reader: ctx.reader,
        target: opts.target,
        from: ctx.buyer,
        functionData: opts.data,
        relayerUrl: ctx.relayerUrl,
        fetchImpl: ctx.fetchImpl,
        onSigned: opts.onSigned,
      });
      opts.onBroadcast?.(txHash);
      await confirmMetaTx(txHash, opts.what, { chainId: opts.target.chainId, reader: ctx.reader, ...ctx.wait });
      return txHash;
    } catch (e) {
      if (isSignatureDeclined(e) || isMetaTxPending(e)) throw e;
      if (isGaslessUnavailable(e) && e.reason === "relayer-unreachable") throw e;
      if (!isGaslessUnavailable(e) || isRequestLimited(e) || !canPayGasItself(ctx.walletKind) || ctx.relayOnly) throw e;
    }
  }
  return sendAndConfirm({
    provider: ctx.provider,
    reader: ctx.reader,
    chainId: opts.target.chainId,
    from: ctx.buyer,
    to: opts.target.address,
    data: opts.data,
    onHandoff: opts.onSigned,
    onBroadcast: opts.onBroadcast,
    wait: ctx.wait,
    what: opts.what,
  });
}

/** Listings on ONE marketplace (all discounted or none), paid in MANA. */
export async function acceptPayingMana(
  ctx: RailContext,
  opts: { trades: SignedTrade[]; coupons?: ListingCoupon[]; requiredManaWei?: bigint; onSigned?: () => void; onBroadcast?: (txHash: Hex) => void },
): Promise<Hex> {
  const first = opts.trades[0];
  if (!first) throw new Error("No items to buy");
  const market = getMarketplaceForTrade(first);
  if (!market) throw new Error("The listing names no marketplace deployed on its chain");
  if (opts.coupons?.length && opts.coupons.length !== opts.trades.length) {
    throw new Error(`acceptWithCoupon needs one coupon per trade, got ${opts.coupons.length} for ${opts.trades.length} trades`);
  }
  await ensureManaAllowance(ctx, first.chainId, market.address, opts.requiredManaWei);
  const data = encodeMarketplaceCall(opts.trades, ctx.buyer, opts.coupons);
  return relayOrSend(ctx, {
    target: { address: market.address, chainId: market.chainId, name: market.name, version: market.version, metaTx: "functionData" },
    data,
    onSigned: opts.onSigned,
    onBroadcast: opts.onBroadcast,
    what: "The MANA purchase",
  });
}

/** CollectionStore mints on one chain, paid in MANA. */
export async function mintPayingMana(
  ctx: RailContext,
  opts: { mints: MintToBuy[]; chainId: ChainId; requiredManaWei?: bigint; onSigned?: () => void; onBroadcast?: (txHash: Hex) => void },
): Promise<Hex> {
  const store = requireContracts(opts.chainId).collectionStore;
  if (!store) throw new Error(`No CollectionStore on chain ${opts.chainId}`);
  const required = opts.requiredManaWei ?? opts.mints.reduce((sum, m) => sum + BigInt(m.item.priceWei), 0n);
  await ensureManaAllowance(ctx, opts.chainId, store.address, required);
  return relayOrSend(ctx, {
    target: { address: store.address, chainId: store.chainId, name: store.name, version: store.version, metaTx: "functionSignature" },
    data: encodeStoreBuy(opts.mints.map((m) => m.item), ctx.buyer),
    onSigned: opts.onSigned,
    onBroadcast: opts.onBroadcast,
    what: "The MANA purchase",
  });
}

/** Listings and mints in MANA: one call per marketplace/discount group and one per mint chain. */
export async function buyManyWithMana(
  ctx: RailContext,
  opts: { trades: SignedTrade[]; coupons?: Record<string, ListingCoupon>; mints?: MintToBuy[]; manaWei?: bigint; onSigned?: () => void },
): Promise<Hex[]> {
  const mints = opts.mints ?? [];
  if (opts.trades.length === 0 && mints.length === 0) throw new Error("No items to buy");
  const groups = new Map<string, SignedTrade[]>();
  for (const t of opts.trades) {
    const key = `${t.chainId}:${t.contract.toLowerCase()}:${opts.coupons?.[t.id] ? "coupon" : "plain"}`;
    groups.set(key, [...(groups.get(key) ?? []), t]);
  }
  const mintGroups = new Map<number, MintToBuy[]>();
  for (const m of mints) mintGroups.set(m.chainId, [...(mintGroups.get(m.chainId) ?? []), m]);
  const hashes: Hex[] = [];
  for (const group of groups.values()) {
    const coupons = opts.coupons ? group.map((t) => opts.coupons![t.id]).filter((c): c is ListingCoupon => c != null) : [];
    hashes.push(await acceptPayingMana(ctx, { trades: group, coupons: coupons.length ? coupons : undefined, requiredManaWei: opts.manaWei, onSigned: opts.onSigned }));
  }
  for (const [chainId, group] of mintGroups) {
    hashes.push(await mintPayingMana(ctx, { mints: group, chainId, onSigned: opts.onSigned }));
  }
  return hashes;
}

export async function buyWithMana(ctx: RailContext, opts: { trade: SignedTrade; coupon?: ListingCoupon; manaWei?: bigint; onSigned?: () => void }): Promise<Hex> {
  const [hash] = await buyManyWithMana(ctx, { trades: [opts.trade], coupons: opts.coupon ? { [opts.trade.id]: opts.coupon } : undefined, manaWei: opts.manaWei, onSigned: opts.onSigned });
  return hash!;
}

export async function buyMintWithMana(ctx: RailContext, opts: { mint: MintToBuy; onSigned?: () => void }): Promise<Hex> {
  const [hash] = await buyManyWithMana(ctx, { trades: [], mints: [opts.mint], onSigned: opts.onSigned });
  return hash!;
}

/**
 * The terms a credits + MANA purchase settles on, checked against the credit the node returned. For a line quoted
 * `quotedCents` whose sale moves `priceWei` MANA (a mint's price, or a MANA listing's after its coupon), the credit for
 * the Credits part `partCents` must be exactly `C = floor(priceWei * partCents / quotedCents)`, the node's own sizing;
 * the buyer's MANA is then `G = priceWei - C`. Anything else stops the purchase before it is sent: a credit that is not
 * that share, no MANA left to pay, or more than the buyer confirmed (`confirmedWei`). Both caps are encoded from these.
 */
export function splitTerms(opts: {
  priceWei: bigint;
  quotedCents: number;
  partCents: number;
  credit: { amount: string; availableAmount: string };
  confirmedWei: bigint;
}): { creditWei: bigint; gapWei: bigint } {
  const { priceWei, quotedCents, partCents } = opts;
  if (!Number.isSafeInteger(quotedCents) || !Number.isSafeInteger(partCents) || partCents <= 0 || partCents >= quotedCents || priceWei <= 0n) {
    throw new Error("The Credits part is not inside the price.");
  }
  const share = (priceWei * BigInt(partCents)) / BigInt(quotedCents);
  if (share <= 0n) throw new Error("The Credits part is too small to carry any MANA.");
  if (BigInt(opts.credit.amount) !== share || BigInt(opts.credit.availableAmount) !== share) {
    throw new Error("The credit is not the share of the price it should be.");
  }
  const gapWei = priceWei - share;
  if (gapWei <= 0n) throw new Error("No MANA is left to pay: the terms changed.");
  if (gapWei > opts.confirmedWei) throw new Error("The MANA this needs moved past the amount shown.");
  return { creditWei: share, gapWei };
}

/**
 * The purchase stopped being its caller's (`RailContext.live`: its dialog closed, or its terms changed) before its sale was
 * kept or sent: after its approval, or while its own signature was asked. Nothing of the sale was sent, and its credit is
 * released like any purchase that failed before it left.
 */
export class SaleCancelledError extends Error {
  constructor() {
    super("The purchase was closed before it was sent, so nothing was bought.");
    this.name = "SaleCancelledError";
  }
}

/**
 * The call a split's sale makes: the CreditsManager it goes to, the external call it runs there, and the useCredits call
 * itself, encoded (`useCredits`), exactly as either rail sends it (the relay inside its meta-transaction).
 */
export type SplitDispatch = { manager: string; target: Hex; selector: Hex; callData: Hex; useCredits: Hex };

type GapOptions = {
  credits: SpendableCredit[];
  manaGapWei: bigint;
  /**
   * Right before the sale goes to the relay or the wallet, once the allowance is in place: from here on it may be sent,
   * so a failure after this is a sale whose outcome must be found out, not one that never left. It is given the call the
   * sale makes (its CreditsManager and its external call), which the caller keeps to tell this sale's execution apart
   * (splitEvidence.ts). A throw here stops the sale before anything is sent (the caller could not record it).
   */
  onDispatch?: (call: SplitDispatch) => void;
  /**
   * The exact bytes the sale leaves as, right before they leave: the relayed execution as signed (the call data the relay
   * sends to the manager), or the useCredits call the wallet is handed (after a relay that proved it sent nothing, these
   * are the ones that may leave). The caller keeps their identity with the sale (`splitEnvelope`): only a revert of these
   * very bytes says this sale failed. A throw here stops them unsent.
   */
  onEnvelope?: (dispatched: Hex) => void;
  onBroadcast?: (info: { txHash: Hex }) => void;
  onReverted?: (info: { txHash: string | null }) => void;
  onUnobservable?: () => void;
};

/**
 * Credits first, MANA for the remainder, in ONE useCredits: the CreditsManager pulls the gap from the
 * buyer's MANA, so its allowance is exactly the gap. The two caps are encoded apart: `maxCreditedValue`
 * is the credit's own value, `maxUncreditedValue` the gap the buyer confirmed, never raised here. Only
 * ever reached from the MANA option of an eligible primary line.
 */
async function payGapWithMana(ctx: RailContext, target: PurchaseTarget, opts: GapOptions): Promise<Hex> {
  if (opts.credits.length === 0) throw new Error("No credits to spend: use the MANA-only rail for that");
  if (opts.manaGapWei <= 0n) throw new Error("No MANA gap to cover: use the credits-only rail for that");
  const chainId = targetChainId(target);
  // The book's manager, which pulls the gap: a credit signed for any other is refused before this approves.
  const spender = creditsManagerFor(chainId, opts.credits).address;
  await ensureManaAllowance(ctx, chainId, spender, opts.manaGapWei, true);
  // Its approval may have taken a while (signed, relayed, confirmed): a purchase gone meanwhile is neither kept nor signed.
  if (ctx.live?.() === false) throw new SaleCancelledError();
  const creditsValue = opts.credits.reduce((acc, c) => acc + BigInt(c.availableAmount), 0n);
  const purchase: AnyPurchase = { ...purchaseFor(target, opts.credits, creditsValue.toString()), maxUncreditedValue: opts.manaGapWei.toString() };
  // The call both rails send (the relay's meta-transaction wraps it; the wallet sends it as is), built once: what the caller
  // keeps of it is what leaves, never a second build (its external call carries a salt of its own each time).
  const built = buildGroupUseCreditsArgs(groupPurchases([purchase])[0]!, ctx.buyer);
  const call = built.args.externalCall;
  opts.onDispatch?.({ manager: built.manager.address, target: call.target, selector: call.selector, callData: call.data, useCredits: built.data });
  if (ctx.gasless ?? gaslessEnabled()) {
    let relayed: Hex | undefined;
    try {
      relayed = await buyOneGasless(ctx, {
        purchase,
        built,
        // Signed, not yet kept or sent: a purchase gone while its wallet prompt was open sends nothing (as its approval).
        onSignedCall: ({ data }) => {
          if (ctx.live?.() === false) throw new SaleCancelledError();
          opts.onEnvelope?.(data);
        },
      });
      opts.onBroadcast?.({ txHash: relayed });
      await waitForSettlement(ctx.reader, chainId, relayed, ctx.wait);
      return relayed;
    } catch (e) {
      // The relay answered its hash: only that hash's receipt, mined and reverted, may say the credit went unspent (the caller
      // checks it carries the bytes this sale left as, `onEnvelope`). Any other failure (a receipt read aborted or failed, no
      // receipt in time) leaves the sale pending, never sent again.
      if (relayed) {
        if (isRevertedTxError(e) && revertedTxHash(e)?.toLowerCase() === relayed.toLowerCase()) opts.onReverted?.({ txHash: relayed });
        throw e;
      }
      if (isSignatureDeclined(e) || isSettlementPending(e) || e instanceof TxPendingError) throw e;
      if (!isGaslessUnavailable(e)) throw e;
      // Only a relay that proved nothing was sent lets the wallet send the sale instead: any other failure (a lost
      // answer, an unknown one) may have sent it, so its credit may be consumed and it is not sent twice.
      if (!provedNotSent(e)) {
        opts.onUnobservable?.();
        throw e;
      }
      if (isRequestLimited(e) || !canPayGasItself(ctx.walletKind)) throw e;
    }
  }
  let handedOff = false;
  try {
    return await buyOneWithCredits(ctx, {
      purchase,
      built,
      onHandoff: (data) => {
        // Right before the wallet is asked to send it: a purchase gone by now isn't handed over.
        if (ctx.live?.() === false) throw new SaleCancelledError();
        opts.onEnvelope?.(data);
        handedOff = true;
      },
      onBroadcast: opts.onBroadcast,
      onReverted: opts.onReverted,
    });
  } catch (e) {
    // Handed to the wallet, the sale may have gone out though no hash came back (a wallet that disconnected after
    // broadcasting answers 4900): only the buyer's refusal or a mined revert proves its credit untouched.
    if (handedOff && !isWalletRefusal(e) && !isRevertedTxError(e)) opts.onUnobservable?.();
    throw e;
  }
}

export function buyWithCreditsAndMana(ctx: RailContext, opts: GapOptions & { trade: SignedTrade; coupon?: ListingCoupon }): Promise<Hex> {
  return payGapWithMana(ctx, { kind: "trade", trade: opts.trade, coupon: opts.coupon }, opts);
}

export function buyMintWithCreditsAndMana(ctx: RailContext, opts: GapOptions & { mint: MintToBuy }): Promise<Hex> {
  return payGapWithMana(ctx, { kind: "store", mint: opts.mint }, opts);
}
