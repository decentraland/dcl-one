// from: shop@94584a5c:app/src/lib/authorizations.ts (getManaSpendingAuthorization, needsApprovalStep),
//       app/src/hooks/useManaBalance.ts; marketplace@f3956826:webapp/src/modules/bid/utils.ts (isInsufficientMANA)
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.
// MANA the wallet holds and has allowed a contract to spend, read from the target chain, and the one
// allowance a MANA bid or a MANA purchase needs before it can settle. On Polygon the approval is relayed
// (MANA (PoS) supports native meta-transactions); on Ethereum the wallet sends it and pays gas.

import { encodeFunctionData } from "viem";

import type { Eip1193Provider } from "../../../data/auth/wallet";
import { isPolygon, type ChainId } from "@shop/model/chain";
import type { ManaWei } from "@shop/model/money";
import { ERC20_ABI, manaToken } from "@shop/market/lib/contracts";
import { readNonce, sendMetaTransaction } from "./metaTx";
import { readContract, rpc } from "./rpc";
import { ApprovalBusyError, bounded, isTxPending, READ_TIMEOUT_MS, sendCall, TxPendingError, waitForReceipt, walletAccount, WalletAccountChangedError, type WaitOptions } from "./tx";

export { parseManaInput } from "@shop/model/money";

export async function readManaBalance(chainId: ChainId, owner: string): Promise<ManaWei> {
  const mana = manaToken(chainId);
  return (await readContract({ chainId, address: mana.address, abi: ERC20_ABI, functionName: "balanceOf", args: [owner] })) as ManaWei;
}

export async function readManaAllowance(chainId: ChainId, owner: string, spender: string): Promise<bigint> {
  const mana = manaToken(chainId);
  return (await readContract({ chainId, address: mana.address, abi: ERC20_ABI, functionName: "allowance", args: [owner, spender] })) as bigint;
}

/** Whether `spender` may already move `amount` of the owner's MANA (no approval step needed). */
export async function hasManaAllowance(chainId: ChainId, owner: string, spender: string, amount: bigint): Promise<boolean> {
  return (await readManaAllowance(chainId, owner, spender)) >= amount;
}

/**
 * An approval this page sent whose end it has not seen (its wait ran out), per chain, owner and spender, with the
 * meta-transaction nonce it signed when it was relayed. Kept until its own receipt says it mined or reverted, or until
 * the owner's meta-transaction nonce is past the one it signed (it can then never run: it ran, under this hash or a
 * relay's replacement, or another write took that nonce). Never released by time: while it stands, no second approval
 * goes out for that spender (it would only cost a second fee), and a purchase the allowance does not cover stops on its
 * hash. It lives in this page's memory: a direct (Ethereum) approval dropped or replaced without a receipt holds a short
 * allowance back until the page is reloaded.
 */
type Unconfirmed = { hash: string; nonce?: bigint };
const unconfirmedApprovals = new Map<string, Unconfirmed>();
/** One approval decision at a time per key: what the next decision on that key waits for. */
const approvalQueue = new Map<string, Promise<unknown>>();
const approvalKey = (chainId: ChainId, owner: string, spender: string) => `${chainId}:${owner}:${spender.toLowerCase()}`;

/** How long a purchase waits for an earlier approval decision on the same key (which may hold a wallet prompt). */
const QUEUE_WAIT_MS = 60_000;

export function resetUnconfirmedApprovalsForTests(): void {
  unconfirmedApprovals.clear();
  approvalQueue.clear();
}

/** Whether an earlier approval has ended, read within `ms` each: its receipt, else its meta-transaction nonce. */
async function approvalEnded(chainId: ChainId, owner: string, record: Unconfirmed, ms: number): Promise<boolean> {
  try {
    const receipt = await bounded(rpc<{ status?: string } | null>(chainId, "eth_getTransactionReceipt", [record.hash]), ms);
    if (receipt?.status === "0x1" || receipt?.status === "0x0") return true;
  } catch {
    // No answer in time: the nonce may still settle it.
  }
  if (record.nonce === undefined) return false;
  try {
    return (await bounded(readNonce(manaToken(chainId), owner), ms)) > record.nonce;
  } catch {
    return false;
  }
}

/**
 * Runs `decide` once every decision already queued on `key` has ended, queued synchronously so two callers never
 * interleave. One that waits longer than `waitMs` ends with ApprovalBusyError, unsent; the decisions after it still
 * wait for the earlier ones, so giving up never lets a later decision past a stuck one.
 */
function serialized<T>(key: string, decide: () => Promise<T>, waitMs: number): Promise<T> {
  const prior = approvalQueue.get(key) ?? Promise.resolve();
  const mine = bounded(prior, waitMs).then(decide, () => {
    throw new ApprovalBusyError();
  });
  const tail = Promise.all([prior, mine.catch(() => {})]);
  approvalQueue.set(key, tail);
  void tail.then(() => {
    if (approvalQueue.get(key) === tail) approvalQueue.delete(key);
  });
  return mine;
}

/**
 * Approves `spender` for `amount` MANA when the current allowance is short. Resolves null when nothing was
 * needed, else the confirmed hash. The exact amount, never an unlimited allowance. Decisions for one chain, owner
 * and spender run one after another, each reading its own allowance, and each bound to the account it started on:
 * a wallet on another account, or none, ends it before any prompt. While an earlier approval for that spender has
 * not been seen to end, a purchase its allowance does not cover throws `TxPendingError` with the earlier hash. Every
 * read is bounded (`wait.readTimeoutMs`): one that fails before sending sends nothing.
 */
export async function ensureManaAllowance(opts: {
  provider: Eip1193Provider;
  chainId: ChainId;
  spender: string;
  amount: bigint;
  wait?: WaitOptions;
  /** The buyer the caller's purchase is bound to (checked against the wallet when its decision starts); the wallet's account when absent. */
  owner?: string;
  /** How long to wait behind an earlier decision on the same key; tests shorten it. */
  queueTimeoutMs?: number;
}): Promise<string | null> {
  const ms = opts.wait?.readTimeoutMs ?? READ_TIMEOUT_MS;
  const owner = opts.owner?.toLowerCase() ?? (await bounded(walletAccount(opts.provider), ms));
  const key = approvalKey(opts.chainId, owner, opts.spender);
  return serialized(key, () => decideApproval(opts, owner, key), opts.queueTimeoutMs ?? QUEUE_WAIT_MS);
}

/** Throws unless the wallet is still on `owner`, read within `ms` (a disconnected wallet throws walletAccount's own error). */
async function stillOn(provider: Eip1193Provider, owner: string, ms: number): Promise<void> {
  if ((await bounded(walletAccount(provider), ms)) !== owner) throw new WalletAccountChangedError();
}

async function decideApproval(
  opts: { provider: Eip1193Provider; chainId: ChainId; spender: string; amount: bigint; wait?: WaitOptions },
  owner: string,
  key: string,
): Promise<string | null> {
  const ms = opts.wait?.readTimeoutMs ?? READ_TIMEOUT_MS;
  await stillOn(opts.provider, owner, ms);
  const covered = () => bounded(hasManaAllowance(opts.chainId, owner, opts.spender, opts.amount), ms);
  const earlier = unconfirmedApprovals.get(key);
  if (earlier) {
    if (!(await approvalEnded(opts.chainId, owner, earlier, ms))) {
      // A covering allowance lets this purchase pass, but says nothing about the earlier approval: it stays recorded.
      if (await covered().catch(() => false)) {
        await stillOn(opts.provider, owner, ms);
        return null;
      }
      throw new TxPendingError(earlier.hash, "The MANA approval");
    }
    // Ended: it no longer holds a second approval back.
    if (unconfirmedApprovals.get(key) === earlier) unconfirmedApprovals.delete(key);
  }
  let enough: boolean;
  try {
    enough = await covered();
  } catch {
    throw new Error("We couldn't read your MANA allowance. Nothing was sent; try again in a moment.");
  }
  if (enough) {
    await stillOn(opts.provider, owner, ms);
    return null;
  }
  const mana = manaToken(opts.chainId);
  let record: Unconfirmed;
  if (isPolygon(opts.chainId)) {
    // The nonce it signs is recorded with it: once the owner's nonce is past it, the approval can never run.
    const nonce = await bounded(readNonce(mana, owner), ms);
    const hash = await sendMetaTransaction({
      provider: opts.provider,
      contract: mana,
      kind: "collection",
      functionData: encodeFunctionData({ abi: ERC20_ABI, functionName: "approve", args: [opts.spender as `0x${string}`, opts.amount] }),
      nonce,
      account: owner,
      readTimeoutMs: ms,
    });
    record = { hash, nonce };
  } else {
    const hash = await sendCall(opts.provider, { chainId: opts.chainId, to: mana.address, abi: ERC20_ABI, functionName: "approve", args: [opts.spender, opts.amount], account: owner, readTimeoutMs: ms });
    record = { hash };
  }
  unconfirmedApprovals.set(key, record);
  try {
    const confirmed = await waitForReceipt(opts.chainId, record.hash, "The MANA approval", opts.wait);
    if (unconfirmedApprovals.get(key) === record) unconfirmedApprovals.delete(key);
    return confirmed;
  } catch (e) {
    // Reverted: ended. Still pending (an unanswered read included): kept for the next decision to reconcile.
    if (!isTxPending(e) && unconfirmedApprovals.get(key) === record) unconfirmedApprovals.delete(key);
    throw e;
  }
}
