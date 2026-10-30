// from: shop@94584a5c:app/src/lib/buy.ts
// The credits rail: CreditsManager.useCredits wrapping one external call (accept / acceptWithCoupon on
// one marketplace, or CollectionStore.buy), submitted by the buyer's wallet. Grouping here is the same
// identity the relayed rail and the cart use, so the number of confirmations a buyer is told about is
// computed by the code that creates the transactions. Broadcast, settled and reverted are reported as
// three distinct facts: only a mined revert makes a broadcast credit safe to release.

import type { Hex } from "viem";

import type { ChainId } from "../../model/chain";
import { requireContracts, type ContractInfo } from "../chain/contracts";
import { isRevertedTxError, revertedTxHash, sendAndConfirm } from "../chain/wallet";
import {
  buildStoreUseCreditsArgs,
  buildUseCreditsArgs,
  encodeUseCredits,
  type CreditPurchase,
  type EnvelopeOptions,
  type ListingCoupon,
  type SignedTrade,
  type SpendableCredit,
  type StorePurchase,
  type UseCreditsArgs,
} from "./encoding";
import type { RailContext } from "./rail";
import { reportSubmittedTx } from "./report";

export { isRevertedTxError, revertedTxHash };
export type { CreditPurchase, ListingCoupon, SpendableCredit, StorePurchase };

export type AnyPurchase = ({ kind: "trade" } & CreditPurchase) | ({ kind: "store" } & StorePurchase);

export type PurchaseGroup =
  | { kind: "trade"; chainId: ChainId; marketplace: string; purchases: CreditPurchase[] }
  | { kind: "store"; chainId: ChainId; purchases: StorePurchase[] };

/** Which transaction a purchase settles in: store per chain; trades per chain, marketplace, discounted or not. */
export function purchaseGroupKey(
  purchase: AnyPurchase | { kind: "store"; chainId: number } | { kind: "trade"; trade: { chainId: number; contract: string }; coupon?: unknown },
): string {
  if (purchase.kind === "store") return `store:${purchase.chainId}`;
  const discounted = "coupon" in purchase && purchase.coupon ? "coupon" : "plain";
  return `trade:${purchase.trade.chainId}:${purchase.trade.contract.toLowerCase()}:${discounted}`;
}

/** The minimum number of transactions: one external call each. */
export function groupPurchases(purchases: AnyPurchase[]): PurchaseGroup[] {
  const groups = new Map<string, PurchaseGroup>();
  for (const p of purchases) {
    const key = purchaseGroupKey(p);
    const existing = groups.get(key);
    if (p.kind === "store") {
      if (existing?.kind === "store") existing.purchases.push(p);
      else groups.set(key, { kind: "store", chainId: p.chainId, purchases: [p] });
    } else if (existing?.kind === "trade") existing.purchases.push(p);
    else groups.set(key, { kind: "trade", chainId: p.trade.chainId, marketplace: p.trade.contract.toLowerCase(), purchases: [p] });
  }
  return [...groups.values()];
}

/** Summed once per credit, not per line: lines of one group share one credit and one signed cap. */
function groupMaxCreditedValue(purchases: { maxCreditedValue: string; credits: { id: string }[] }[]): string {
  const counted = new Set<string>();
  let total = 0n;
  for (const p of purchases) {
    const key = p.credits.map((c) => c.id).join(",");
    if (counted.has(key)) continue;
    counted.add(key);
    total += BigInt(p.maxCreditedValue);
  }
  return total.toString();
}

function dedupeCredits<T extends { id: string }>(credits: T[]): T[] {
  const byId = new Map<string, T>();
  for (const c of credits) if (!byId.has(c.id)) byId.set(c.id, c);
  return [...byId.values()];
}

function groupCoupons(purchases: CreditPurchase[]): ListingCoupon[] | undefined {
  const coupons = purchases.map((p) => p.coupon).filter((c): c is ListingCoupon => c != null);
  if (coupons.length === 0) return undefined;
  if (coupons.length !== purchases.length) throw new Error(`a discounted batch needs one coupon per line, got ${coupons.length} for ${purchases.length}`);
  return coupons;
}

/**
 * The CreditsManager a group spends through: the one its credits were signed for (a credit's hash binds
 * the manager's address), else the chain's configured one. Credits signed for different managers cannot
 * share a transaction.
 */
export function creditsManagerFor(chainId: ChainId, credits: SpendableCredit[]): ContractInfo {
  const book = requireContracts(chainId).creditsManager;
  if (!book) throw new Error(`No CreditsManager on chain ${chainId}: credits settle on Polygon only`);
  const signed = [...new Set(credits.map((c) => c.contract?.toLowerCase()).filter((a): a is string => !!a && /^0x[0-9a-f]{40}$/.test(a)))];
  if (signed.length > 1) throw new Error("A group's credits were signed for different CreditsManagers");
  return signed[0] ? { ...book, address: signed[0] as ContractInfo["address"] } : book;
}

/** The useCredits call of ONE group and the credit ids it spends, shared by both rails. */
export function buildGroupUseCreditsArgs(
  group: PurchaseGroup,
  buyer: string,
  env?: EnvelopeOptions,
): { args: UseCreditsArgs; salts: string[]; chainId: ChainId; manager: ContractInfo; data: Hex } {
  const credits = dedupeCredits(group.purchases.flatMap((p) => p.credits));
  const maxCreditedValue = groupMaxCreditedValue(group.purchases);
  const manager = creditsManagerFor(group.chainId, credits);
  let args: UseCreditsArgs;
  if (group.kind === "store") {
    const store = requireContracts(group.chainId).collectionStore;
    if (!store) throw new Error(`No CollectionStore on chain ${group.chainId}`);
    args = buildStoreUseCreditsArgs(store.address, group.purchases.map((p) => p.item), buyer, credits, maxCreditedValue, env);
  } else {
    args = buildUseCreditsArgs(group.marketplace, group.purchases.map((p) => p.trade), buyer, credits, maxCreditedValue, groupCoupons(group.purchases), env);
  }
  return { args, salts: credits.map((c) => c.id), chainId: group.chainId, manager, data: encodeUseCredits(args) };
}

/** Submit a built useCredits from the buyer's wallet (requires the chain; never switches it). */
export async function sendUseCredits(
  ctx: RailContext,
  opts: { chainId: ChainId; manager: ContractInfo; data: Hex; onHandoff?: () => void; onBroadcast?: (txHash: Hex) => void },
): Promise<Hex> {
  return sendAndConfirm({
    provider: ctx.provider,
    reader: ctx.reader,
    chainId: opts.chainId,
    from: ctx.buyer,
    to: opts.manager.address,
    data: opts.data,
    onHandoff: opts.onHandoff,
    onBroadcast: opts.onBroadcast,
    wait: ctx.wait,
    what: "The purchase",
  });
}

/** One purchase (a trade or a mint) in a single useCredits. */
export async function buyOneWithCredits(
  ctx: RailContext,
  opts: { purchase: AnyPurchase; onBroadcast?: (info: { txHash: Hex }) => void; onReverted?: (info: { txHash: string | null }) => void },
): Promise<Hex> {
  if (opts.purchase.credits.length === 0) throw new Error("No credits to spend");
  const built = buildGroupUseCreditsArgs(groupPurchases([opts.purchase])[0]!, ctx.buyer);
  try {
    return await sendUseCredits(ctx, {
      ...built,
      onBroadcast: (txHash) => {
        opts.onBroadcast?.({ txHash });
        void reportSubmittedTx({ txHash, salts: built.salts, client: ctx.client });
      },
    });
  } catch (err) {
    if (isRevertedTxError(err)) opts.onReverted?.({ txHash: revertedTxHash(err) });
    throw err;
  }
}

/** A listed trade bought with credits; `maxCreditedValue` is what the server sized for it. */
export async function buyWithCredits(
  ctx: RailContext,
  opts: { trade: SignedTrade; credits: SpendableCredit[]; maxCreditedValue: string; coupon?: ListingCoupon; onBroadcast?: (info: { txHash: Hex }) => void; onReverted?: (info: { txHash: string | null }) => void },
): Promise<Hex> {
  if (opts.credits.length === 0) throw new Error("No credits to spend");
  return buyOneWithCredits(ctx, {
    purchase: { kind: "trade", trade: opts.trade, coupon: opts.coupon, credits: opts.credits, maxCreditedValue: opts.maxCreditedValue },
    onBroadcast: opts.onBroadcast,
    onReverted: opts.onReverted,
  });
}

export type GroupCallbacks = {
  /** Right before a group's transaction is handed to the wallet. */
  onHandoff?: (group: number, total: number) => void;
  onSigned?: (signed: number, total: number) => void;
  onBroadcast?: (info: { txHash: Hex; salts: string[] }) => void;
  onSettled?: (info: { txHash: Hex; salts: string[] }) => void;
  onReverted?: (info: { salts: string[] }) => void;
};

/** Several purchases in as few signatures as possible, group by group; returns the hashes in group order. */
export async function buyManyWithCredits(ctx: RailContext, opts: { purchases: AnyPurchase[] } & GroupCallbacks): Promise<Hex[]> {
  if (opts.purchases.length === 0) throw new Error("No items to buy");
  const groups = groupPurchases(opts.purchases);
  const hashes: Hex[] = [];
  for (const group of groups) {
    const built = buildGroupUseCreditsArgs(group, ctx.buyer);
    let hash: Hex;
    try {
      hash = await sendUseCredits(ctx, {
        ...built,
        onHandoff: () => opts.onHandoff?.(hashes.length + 1, groups.length),
        onBroadcast: (txHash) => {
          opts.onSigned?.(hashes.length + 1, groups.length);
          opts.onBroadcast?.({ txHash, salts: built.salts });
          void reportSubmittedTx({ txHash, salts: built.salts, client: ctx.client });
        },
      });
    } catch (err) {
      if (isRevertedTxError(err)) opts.onReverted?.({ salts: built.salts });
      throw err;
    }
    hashes.push(hash);
    opts.onSettled?.({ txHash: hash, salts: built.salts });
  }
  return hashes;
}
