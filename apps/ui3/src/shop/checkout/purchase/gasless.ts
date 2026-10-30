// from: shop@94584a5c:app/src/lib/buy-gasless.ts
// The relayed credits rail: the buyer signs an EIP-712 MetaTransaction wrapping the SAME useCredits
// calldata the direct rail sends (credits.ts builds both), and the relay submits and pays. Works from
// any network the wallet is on, so a web2 buyer never meets gas or a chain. A mixed basket signs once
// per group, and each signature is only valid against the nonce the contract holds at that moment, so
// the next group waits for the nonce to advance (hash-agnostic: the relay re-submits with other hashes).
// Outcomes a caller must tell apart: confirmed; reverted (credits untouched, releasable);
// SettlementPendingError (broadcast, may still land: KEEP the reservation, never retry on another rail).

import type { Hex } from "viem";

import type { ChainId } from "../../model/chain";
import { gaslessEnabled } from "../chain/config";
import type { ContractInfo } from "../chain/contracts";
import { GaslessUnavailableError, readMetaTxNonce, relayMetaTransaction, type MetaTxTarget } from "../chain/metatx";
import { defaultSleep, waitForReceipt, type ChainReader } from "../chain/rpc";
import { TxRevertedError } from "../chain/wallet";
import { buildGroupUseCreditsArgs, groupPurchases, type AnyPurchase } from "./credits";
import type { UseCreditsArgs } from "./encoding";
import { encodeUseCredits } from "./encoding";
import type { RailContext } from "./rail";
import { reportSubmittedTx } from "./report";

export { GaslessUnavailableError };

/** Broadcast by the relay but not confirmed in the window: it may still mine. */
export class SettlementPendingError extends Error {
  override name = "SettlementPendingError";
  constructor(
    readonly txHash: string,
    readonly reason?: unknown,
  ) {
    super("Purchase not yet confirmed");
  }
}

export function isSettlementPending(e: unknown): e is SettlementPendingError {
  return e instanceof SettlementPendingError || (e as { name?: string } | null)?.name === "SettlementPendingError";
}

function managerTarget(manager: ContractInfo): MetaTxTarget {
  return { address: manager.address, chainId: manager.chainId, name: manager.name, version: manager.version, metaTx: "functionData" };
}

function enabled(ctx: RailContext): boolean {
  return ctx.gasless ?? gaslessEnabled();
}

/**
 * Resolve with the manager's nonce once it moved past `signedNonce`, or null inside the window. Any
 * advance counts (another tab, an approval): the next group signs against whatever the chain now holds.
 */
export async function waitForNonceAdvance(opts: {
  reader: ChainReader;
  target: Pick<MetaTxTarget, "address" | "chainId" | "metaTx">;
  buyer: string;
  signedNonce: bigint;
  timeoutMs?: number;
  pollMs?: number;
  now?: () => number;
  sleep?: (ms: number) => Promise<void>;
}): Promise<bigint | null> {
  const timeoutMs = opts.timeoutMs ?? 120_000;
  const pollMs = Math.max(opts.pollMs ?? 1_500, 10);
  const now = opts.now ?? Date.now;
  const sleep = opts.sleep ?? defaultSleep;
  const started = now();
  for (;;) {
    try {
      const nonce = await readMetaTxNonce(opts.reader, opts.target, opts.buyer);
      if (nonce > opts.signedNonce) return nonce;
    } catch {
      // A failed read is not an answer.
    }
    if (now() - started >= timeoutMs) return null;
    await sleep(pollMs);
  }
}

/** A relayed hash: resolves on success; throws TxRevertedError (final) or SettlementPendingError. */
export async function waitForSettlement(
  reader: ChainReader,
  chainId: ChainId,
  txHash: string,
  opts: { timeoutMs?: number; pollMs?: number; now?: () => number; sleep?: (ms: number) => Promise<void> } = {},
): Promise<void> {
  const receipt = await waitForReceipt(reader, chainId, txHash, { timeoutMs: opts.timeoutMs ?? 120_000, ...opts });
  if (!receipt) throw new SettlementPendingError(txHash);
  if (receipt.status === "reverted") throw new TxRevertedError(receipt, "The purchase");
}

/** Relay an already-built useCredits (a NAME registration's server-signed external call too). */
export async function sendUseCreditsGasless(
  ctx: RailContext,
  opts: { chainId: ChainId; manager: ContractInfo; args: UseCreditsArgs; onSigned?: () => void },
): Promise<Hex> {
  if (!enabled(ctx)) throw new GaslessUnavailableError("gasless checkout disabled", "disabled");
  const { txHash } = await relayMetaTransaction({
    provider: ctx.provider,
    reader: ctx.reader,
    target: managerTarget(opts.manager),
    from: ctx.buyer,
    functionData: encodeUseCredits(opts.args),
    relayerUrl: ctx.relayerUrl,
    fetchImpl: ctx.fetchImpl,
    onSigned: opts.onSigned,
  });
  return txHash;
}

/** One purchase (trade or mint), relayed. */
export async function buyOneGasless(ctx: RailContext, opts: { purchase: AnyPurchase; onSigned?: () => void }): Promise<Hex> {
  if (!enabled(ctx)) throw new GaslessUnavailableError("gasless checkout disabled", "disabled");
  if (opts.purchase.credits.length === 0) throw new Error("No credits to spend");
  const built = buildGroupUseCreditsArgs(groupPurchases([opts.purchase])[0]!, ctx.buyer);
  const txHash = await sendUseCreditsGasless(ctx, { chainId: built.chainId, manager: built.manager, args: built.args, onSigned: opts.onSigned });
  void reportSubmittedTx({ txHash, salts: built.salts, client: ctx.client });
  return txHash;
}

/** Upstream `buyGasless`: a single listed trade. */
export async function buyGasless(ctx: RailContext, opts: Omit<Extract<AnyPurchase, { kind: "trade" }>, "kind">): Promise<Hex> {
  return buyOneGasless(ctx, { purchase: { kind: "trade", ...opts } });
}

/** Group by group, relayed; the next group waits for the nonce. Returns the hashes in group order. */
export async function buyManyGasless(
  ctx: RailContext,
  opts: {
    purchases: AnyPurchase[];
    onSigned?: () => void;
    onBroadcast?: (info: { txHash: Hex; salts: string[] }) => void;
    onGroupSettling?: (progress: { settled: number; total: number }) => void;
  },
): Promise<Hex[]> {
  if (!enabled(ctx)) throw new GaslessUnavailableError("gasless checkout disabled", "disabled");
  if (opts.purchases.length === 0) throw new Error("No items to buy");
  const groups = groupPurchases(opts.purchases);
  const hashes: Hex[] = [];
  let seen: bigint | undefined;
  for (const [index, group] of groups.entries()) {
    const built = buildGroupUseCreditsArgs(group, ctx.buyer);
    const target = managerTarget(built.manager);
    const { txHash, nonce } = await relayMetaTransaction({
      provider: ctx.provider,
      reader: ctx.reader,
      target,
      from: ctx.buyer,
      functionData: built.data,
      relayerUrl: ctx.relayerUrl,
      fetchImpl: ctx.fetchImpl,
      minNonce: seen,
      onSigned: opts.onSigned,
    });
    opts.onBroadcast?.({ txHash, salts: built.salts });
    void reportSubmittedTx({ txHash, salts: built.salts, client: ctx.client });
    hashes.push(txHash);
    if (index === groups.length - 1) continue;
    opts.onGroupSettling?.({ settled: index + 1, total: groups.length });
    const advanced = await waitForNonceAdvance({ reader: ctx.reader, target, buyer: ctx.buyer, signedNonce: nonce, ...ctx.wait });
    if (advanced === null) throw new SettlementPendingError(txHash);
    seen = advanced;
  }
  return hashes;
}
