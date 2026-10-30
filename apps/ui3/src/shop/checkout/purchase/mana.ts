// from: shop@94584a5c:app/src/lib/buy-mana.ts
// The MANA rails. MANA alone: marketplace.accept (or acceptWithCoupon) for listings and
// CollectionStore.buy for mints, paid from the buyer's MANA, relayed when possible and otherwise sent
// by the wallet (which must already be on the chain). Credits + MANA (only inside the MANA option of an
// eligible primary line): useCredits with a partial credit and the remainder pulled from the buyer's
// MANA by the CreditsManager. The allowance is checked first and sized to THIS purchase: a leftover
// smaller one would revert the settlement.

import type { Hex } from "viem";

import type { ChainId } from "../../model/chain";
import { gaslessEnabled } from "../chain/config";
import { requireContracts } from "../chain/contracts";
import { isGaslessUnavailable, isSignatureDeclined, relayMetaTransaction, type MetaTxTarget } from "../chain/metatx";
import { sendAndConfirm, TxPendingError } from "../chain/wallet";
import { AuthorizationKind, ensureAuthorization } from "../lib/authorizations";
import { getMarketplaceForTrade } from "../lib/marketplace";
import { canPayGasItself } from "../lib/wallet-kind";
import { buyOneWithCredits, type AnyPurchase } from "./credits";
import { encodeMarketplaceCall, encodeStoreBuy, type ListingCoupon, type SignedTrade, type SpendableCredit, type StoreItemToBuy } from "./encoding";
import { buyOneGasless, isSettlementPending, waitForSettlement } from "./gasless";
import type { RailContext } from "./rail";
import { confirmMetaTx, isMetaTxPending } from "./tx-confirm";

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

async function ensureManaAllowance(ctx: RailContext, chainId: ChainId, spender: string, requiredWei: bigint | undefined): Promise<void> {
  await ensureAuthorization({
    auth: { kind: AuthorizationKind.Allowance, contractAddress: requireContracts(chainId).mana.address, spenderAddress: spender, chainId },
    provider: ctx.provider,
    account: ctx.buyer,
    walletKind: ctx.walletKind,
    reader: ctx.reader,
    relayerUrl: ctx.relayerUrl,
    fetchImpl: ctx.fetchImpl,
    requiredWei,
  });
}

/**
 * Relay `data` to `target` when gasless is on; a declined prompt or a relayed-but-unconfirmed call
 * propagates (never re-submitted directly: that could run it twice). Otherwise the wallet sends it.
 * `onSigned` marks the hand-off: after the relayed signature (right before the relay is called), or
 * right before the wallet is asked to send. From then on the call may be broadcast.
 */
async function relayOrSend(
  ctx: RailContext,
  opts: { target: MetaTxTarget; data: Hex; onSigned?: () => void; onBroadcast?: (txHash: Hex) => void; what: string },
): Promise<Hex> {
  if (ctx.gasless ?? gaslessEnabled()) {
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
      if (!isGaslessUnavailable(e) || !canPayGasItself(ctx.walletKind)) throw e;
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

type GapOptions = {
  credits: SpendableCredit[];
  manaGapWei: bigint;
  onBroadcast?: (info: { txHash: Hex }) => void;
  onReverted?: (info: { txHash: string | null }) => void;
  onUnobservable?: () => void;
};

/**
 * Credits first, MANA for the remainder, in ONE useCredits: the CreditsManager pulls the gap from the
 * buyer's MANA, so its allowance is sized to the gap. Only ever reached from the MANA option of an
 * eligible primary line.
 */
async function payGapWithMana(ctx: RailContext, target: PurchaseTarget, opts: GapOptions): Promise<Hex> {
  if (opts.credits.length === 0) throw new Error("No credits to spend: use the MANA-only rail for that");
  if (opts.manaGapWei <= 0n) throw new Error("No MANA gap to cover: use the credits-only rail for that");
  const chainId = targetChainId(target);
  const { spender } = manaSpenderFor("combined", target);
  await ensureManaAllowance(ctx, chainId, spender, opts.manaGapWei);
  const creditsValue = opts.credits.reduce((acc, c) => acc + BigInt(c.availableAmount), 0n);
  const purchase = purchaseFor(target, opts.credits, (creditsValue + opts.manaGapWei).toString());
  if (ctx.gasless ?? gaslessEnabled()) {
    let relayed: Hex | undefined;
    try {
      relayed = await buyOneGasless(ctx, { purchase });
      opts.onBroadcast?.({ txHash: relayed });
      await waitForSettlement(ctx.reader, chainId, relayed, ctx.wait);
      return relayed;
    } catch (e) {
      if (isSignatureDeclined(e) || isSettlementPending(e) || e instanceof TxPendingError) throw e;
      if (!isGaslessUnavailable(e)) {
        if (relayed) opts.onReverted?.({ txHash: relayed });
        throw e;
      }
      if (e.reason === "relayer-unreachable") {
        opts.onUnobservable?.();
        throw e;
      }
      if (!canPayGasItself(ctx.walletKind)) throw e;
    }
  }
  return buyOneWithCredits(ctx, { purchase, onBroadcast: opts.onBroadcast, onReverted: opts.onReverted });
}

export function buyWithCreditsAndMana(ctx: RailContext, opts: GapOptions & { trade: SignedTrade; coupon?: ListingCoupon }): Promise<Hex> {
  return payGapWithMana(ctx, { kind: "trade", trade: opts.trade, coupon: opts.coupon }, opts);
}

export function buyMintWithCreditsAndMana(ctx: RailContext, opts: GapOptions & { mint: MintToBuy }): Promise<Hex> {
  return payGapWithMana(ctx, { kind: "store", mint: opts.mint }, opts);
}
