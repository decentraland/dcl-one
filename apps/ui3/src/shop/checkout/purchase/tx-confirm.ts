// from: shop@94584a5c:app/src/lib/tx-confirm.ts
// A relayed transaction resolves only when it SUCCEEDED. Two failure modes, kept apart because only one
// is final: a mined revert (nothing happened, ever) and no receipt in the window (it may still land, so
// no compensating action: no re-submit, no release). `confirmMetaTxByEffect` also watches the caller's
// own "is it done?" question, since the relay re-submits with higher fees and the first hash it returned
// may never appear on chain at all.

import type { ChainId } from "../../model/chain";
import { defaultSleep, shopChainReader, waitForReceipt, type ChainReader } from "../chain/rpc";

export class MetaTxRevertedError extends Error {
  override name = "MetaTxRevertedError";
  constructor(
    readonly what: string,
    readonly txHash: string,
  ) {
    super(`${what} reverted on-chain`);
  }
}

export class MetaTxPendingError extends Error {
  override name = "MetaTxPendingError";
  constructor(
    readonly what: string,
    readonly txHash: string,
    readonly detail?: unknown,
  ) {
    super(`${what} was submitted but not confirmed in time`);
  }
}

export function isMetaTxPending(e: unknown): e is MetaTxPendingError {
  return e instanceof MetaTxPendingError || (e as { name?: string } | null)?.name === "MetaTxPendingError";
}

export function isMetaTxReverted(e: unknown): e is MetaTxRevertedError {
  return e instanceof MetaTxRevertedError || (e as { name?: string } | null)?.name === "MetaTxRevertedError";
}

export type ConfirmOptions = {
  chainId: ChainId;
  reader?: ChainReader;
  timeoutMs?: number;
  pollMs?: number;
  signal?: AbortSignal;
  now?: () => number;
  sleep?: (ms: number) => Promise<void>;
};

/** Wait for a relayed hash; resolves only on a successful receipt. */
export async function confirmMetaTx(txHash: string, what: string, opts: ConfirmOptions): Promise<string> {
  const reader = opts.reader ?? shopChainReader();
  const receipt = await waitForReceipt(reader, opts.chainId, txHash, {
    timeoutMs: opts.timeoutMs ?? 120_000,
    pollMs: opts.pollMs,
    signal: opts.signal,
    now: opts.now,
    sleep: opts.sleep,
  });
  if (!receipt) throw new MetaTxPendingError(what, txHash);
  if (receipt.status === "reverted") throw new MetaTxRevertedError(what, txHash);
  return txHash;
}

/**
 * Race the receipt of `txHash` against the caller's own effect check; a revert of `txHash` is still
 * final. Ten minutes by default: a fee-bumped relay on a congested Polygon takes that long.
 */
export async function confirmMetaTxByEffect(
  opts: ConfirmOptions & { txHash: string; what: string; isDone: () => Promise<boolean>; onWaiting?: (elapsedMs: number) => void },
): Promise<string> {
  const { txHash, what, isDone } = opts;
  const reader = opts.reader ?? shopChainReader();
  const timeoutMs = opts.timeoutMs ?? 10 * 60_000;
  const pollMs = Math.max(opts.pollMs ?? 5_000, 10);
  const now = opts.now ?? Date.now;
  const sleep = opts.sleep ?? ((ms: number) => defaultSleep(ms, opts.signal));
  const started = now();
  while (now() - started < timeoutMs) {
    try {
      if (await isDone()) return txHash;
    } catch {
      // A failed read is not an answer.
    }
    try {
      const receipt = await reader.receipt(opts.chainId, txHash);
      if (receipt) {
        if (receipt.status === "reverted") throw new MetaTxRevertedError(what, txHash);
        return txHash;
      }
    } catch (e) {
      if (isMetaTxReverted(e)) throw e;
    }
    opts.onWaiting?.(now() - started);
    await sleep(pollMs);
  }
  try {
    if (await isDone()) return txHash;
  } catch {
    // fall through to pending
  }
  throw new MetaTxPendingError(what, txHash);
}
