// The hold on a split sale whose outcome is unknown (Credits and MANA paying one item together), shared by every surface
// that sends one: the web Shop's dialog (detail/components/CombinedBuy.tsx) and the in-world Shop. It is kept per account
// and cart line in the storage every tab of this browser shares, so it outlives a tab and a restart, and a second sale of
// the same item waits on it whichever surface sent the first.
//
// One run at a time per account and line: `claimSplitHold` takes the line's Web Lock (only if free) and reads the record
// inside it. The run holding the claim writes the record before the sale may leave (`writeSplitHold`: a browser that
// cannot keep it does not send), rewrites it with what it learns, and clears only its own attempt's (`clearSplitHold`). A
// record whose credit is known stays until the sale's receipt, or the node's record of that credit once it can no longer
// be spent, says what became of it (`settleSplitHold`, then `clearSettledSplitHold`): time alone never lets it go. Only a
// record from before credits were kept with it (no credit) is let go once `SPLIT_HOLD_MS` has passed.

import type { ShopClient } from "@shop/api/shop";
import { fetchUserPurchases, type PurchaseRecord } from "@shop/credits/credits";
import type { AssetId } from "@shop/model/asset";
import type { CartListingRef } from "@shop/model/purchase";
import { cartLineId } from "../../cart/store";
import type { ChainReader } from "../chain/rpc";
import { newAttemptId, webLock } from "../lib/web-lock";

const KEY = "dcl-shop-buynow-split:v1:";
/** How long a record from before credits were kept with it holds; and how long Check again waits on a fresh one. */
export const SPLIT_HOLD_MS = 15 * 60 * 1000;
/** The node's purchase history, read this many lines at a time (its most) while looking for one credit. */
const HISTORY_PAGE = 200;

/**
 * A split sent and not settled. `attempt`: the run that wrote it, the only one that changes or clears it. `txHash` and
 * `chainId`: the sale's transaction, once one is known. `creditId`: the credit the split reserved (a sale found reverted
 * retires it). `creditExpiresAt` (unix seconds): when that credit stops being spendable.
 */
export type SplitHold = {
  at: number;
  attempt: string | null;
  txHash: string | null;
  chainId: number | null;
  creditId: string | null;
  creditExpiresAt: number | null;
};

/**
 * The cart line a hold is kept under: the item and its listing, as both the web cart and the in-world Shop's cart key a
 * line (`cartLineId`: the asset key, then the trade id, else the acquisition). The same listing of an item is the same
 * line on both; another listing of it is another line.
 */
export function splitHoldLine(asset: AssetId, listing?: CartListingRef): string {
  return cartLineId(asset, listing);
}

/** The storage key of the hold of `account` (lowercased) on `lineId`. */
export function splitHoldKey(account: string | null, lineId: string): string {
  return `${KEY}${account?.toLowerCase() ?? "anonymous"}:${lineId}`;
}

/** The Web Lock a split of `lineId` by `account` runs and settles under, in every tab and on every surface. */
export function splitHoldLockName(account: string, lineId: string): string {
  return `dcl-shop-buynow-split:${account.toLowerCase()}:${lineId}`;
}

/** The storage every tab of this browser shares; none on a page without it, or where the browser blocks it. */
function holdStorage(): Storage | null {
  try {
    return typeof localStorage !== "undefined" ? localStorage : null;
  } catch {
    return null;
  }
}

/** The record as stored, kept or not. A bare number is the first format's (a timestamp, written before the hash was kept). */
export function readSplitHoldRecord(account: string | null, lineId: string): SplitHold | null {
  try {
    const raw = holdStorage()?.getItem(splitHoldKey(account, lineId)) ?? null;
    const parsed = raw ? (JSON.parse(raw) as unknown) : null;
    if (typeof parsed === "number") return { at: parsed, attempt: null, txHash: null, chainId: null, creditId: null, creditExpiresAt: null };
    const hold = parsed as Partial<SplitHold> | null;
    if (!hold || typeof hold.at !== "number") return null;
    return {
      at: hold.at,
      attempt: typeof hold.attempt === "string" ? hold.attempt : null,
      txHash: typeof hold.txHash === "string" ? hold.txHash : null,
      chainId: typeof hold.chainId === "number" ? hold.chainId : null,
      creditId: typeof hold.creditId === "string" ? hold.creditId : null,
      creditExpiresAt: typeof hold.creditExpiresAt === "number" ? hold.creditExpiresAt : null,
    };
  } catch {
    return null;
  }
}

/** Recorded no later than now and less than `SPLIT_HOLD_MS` ago. */
export const splitHoldFresh = (hold: SplitHold, now: number) => now >= hold.at && now - hold.at < SPLIT_HOLD_MS;
/** The record still holds the item: one whose credit is known until it is settled; one from before, while fresh. */
export const splitHoldKeeps = (hold: SplitHold, now: number) => hold.creditId !== null || splitHoldFresh(hold, now);
/** The split's credit, spent or not, can no longer be spent: only then can the node's record of it settle the split. */
export const splitHoldCreditOver = (hold: SplitHold, now: number) => hold.creditExpiresAt === null || now >= hold.creditExpiresAt * 1000;
/** When a check next has something to do for a split with no hash: its hold and its credit both over. */
export const splitHoldReleasableAt = (hold: SplitHold) => Math.max(hold.at + SPLIT_HOLD_MS, (hold.creditExpiresAt ?? 0) * 1000);

/**
 * Whether a check has something to do now: a transaction to read; or, for a split with no hash to read (an answer lost),
 * its hold and its credit both over, so the node's record of that credit can tell spent from released; or a record from
 * before credits were kept with it whose hold has run out.
 */
export function splitHoldCheckable(hold: SplitHold, now: number = Date.now()): boolean {
  return (!!hold.txHash && hold.chainId !== null) || (!splitHoldFresh(hold, now) && (!!hold.txHash || !hold.creditId || splitHoldCreditOver(hold, now)));
}

/** The split `account` sent for `lineId` and has not settled: kept as `splitHoldKeeps` says. */
export function readSplitHold(account: string | null, lineId: string, now: number = Date.now()): SplitHold | null {
  const hold = readSplitHoldRecord(account, lineId);
  return hold && splitHoldKeeps(hold, now) ? hold : null;
}

/**
 * Writes `hold` unless another attempt's record still holds the item; whether it was kept (written and read back). The
 * run holding the claim calls it before the sale may leave, and sends nothing when it answers false.
 */
export function writeSplitHold(account: string | null, lineId: string, hold: SplitHold): boolean {
  try {
    const storage = holdStorage();
    if (!storage) return false;
    const current = readSplitHoldRecord(account, lineId);
    if (current && current.attempt !== hold.attempt && splitHoldKeeps(current, Date.now())) return false;
    const key = splitHoldKey(account, lineId);
    const value = JSON.stringify(hold);
    storage.setItem(key, value);
    return storage.getItem(key) === value;
  } catch {
    return false;
  }
}

/** Clears the record only while it is `attempt`'s: an older attempt's outcome never wipes a newer one's record. */
export function clearSplitHold(account: string | null, lineId: string, attempt: string | null): void {
  try {
    if (readSplitHoldRecord(account, lineId)?.attempt !== attempt) return;
    holdStorage()?.removeItem(splitHoldKey(account, lineId));
  } catch {
    // Nothing was stored.
  }
}

/** A new run's own id. */
export const newSplitAttempt = newAttemptId;

/**
 * The claim on one account's split of one line. `claimed`: the line was free under its lock, which this run now holds
 * until `release()`, with `attempt` to write its record under. `held`: a split of the line is unsettled (shown, nothing
 * sent; the lock is already let go). `busy`: another window or surface holds the lock. `unguarded`: no Web Locks, so
 * nothing proves a split goes out once, and none is sent.
 */
export type SplitHoldClaim =
  | { kind: "claimed"; attempt: string; release: () => void }
  | { kind: "held"; hold: SplitHold }
  | { kind: "busy" }
  | { kind: "unguarded" };

/** Takes the line's lock (only if free) and reads its record inside it; see `SplitHoldClaim`. */
export async function claimSplitHold(locks: LockManager | null | undefined, account: string, lineId: string): Promise<SplitHoldClaim> {
  if (!locks) return { kind: "unguarded" };
  const release = await webLock(locks, splitHoldLockName(account, lineId));
  if (!release) return { kind: "busy" };
  const hold = readSplitHold(account, lineId);
  if (hold) {
    release();
    return { kind: "held", hold };
  }
  return { kind: "claimed", attempt: newSplitAttempt(), release };
}

/** What became of a held split: spent (completed), reverted (its credit unspent, to retire), or released by the node. */
export type SplitOutcome = { kind: "spent"; txHash: string | null } | { kind: "reverted" } | { kind: "released" };

/** The node's record of one credit (listed by its lines, `<salt>:<position>`), read page by page until found or the end. */
async function findCreditRecord(account: string, creditId: string, client: ShopClient): Promise<PurchaseRecord | null> {
  const salt = creditId.toLowerCase();
  for (let skip = 0; ; ) {
    const { items, total, returned } = await fetchUserPurchases(account, { all: true, first: HISTORY_PAGE, skip }, client);
    const found = items.find((p) => p.id.split(":")[0]!.toLowerCase() === salt);
    if (found) return found;
    // By the rows the server sent: the ones the decoder drops (a status it does not know) still take their place, and a
    // server that sends fewer than asked starts its next page right after them.
    skip += returned;
    if (returned === 0 || skip >= total) return null;
  }
}

/**
 * What became of a held split, from its receipt; else, once its credit can no longer be spent, from the node's record of
 * that credit (SETTLED: spent; EXPIRED: released). Null while nothing says (still reserved, not listed, a failed read),
 * however long ago it was sent. A receipt read that fails is no answer, as no receipt is none: the credit is asked next.
 */
export async function settleSplitHold(hold: SplitHold, deps: { account: string | null; reader: ChainReader; client: ShopClient }): Promise<SplitOutcome | null> {
  if (hold.txHash && hold.chainId !== null) {
    const receipt = await deps.reader.receipt(hold.chainId, hold.txHash).catch(() => null);
    if (receipt) return receipt.status === "reverted" ? { kind: "reverted" } : { kind: "spent", txHash: hold.txHash };
  }
  if (hold.creditId) {
    if (!splitHoldCreditOver(hold, Date.now()) || !deps.account) return null;
    try {
      const credit = await findCreditRecord(deps.account, hold.creditId, deps.client);
      if (credit?.status === "SETTLED") return { kind: "spent", txHash: credit.txHash ?? hold.txHash };
      if (credit?.status === "EXPIRED") return { kind: "released" };
    } catch {
      // Not an answer: still held.
    }
    return null;
  }
  // A record from before credits were kept with it: let go once its hold has run out.
  return splitHoldFresh(hold, Date.now()) ? null : { kind: "released" };
}

/**
 * Applying an outcome settled for `read`. `cleared`: it was still the line's record (the same attempt; for one with no
 * attempt, the same `at`) and is gone. `replaced`: another run settled it and wrote another meanwhile (`current`, kept, or
 * null): the outcome is not applied to that one. `busy`: a run holds the line's lock (or there are no Web Locks); try later.
 */
export type SplitHoldClearing = { kind: "cleared" } | { kind: "replaced"; current: SplitHold | null } | { kind: "busy" };

/** Clears the settled `read` under the line's lock, only while it is still the record; see `SplitHoldClearing`. */
export async function clearSettledSplitHold(locks: LockManager | null | undefined, account: string, lineId: string, read: SplitHold): Promise<SplitHoldClearing> {
  const release = await webLock(locks, splitHoldLockName(account, lineId));
  if (!release) return { kind: "busy" };
  try {
    const current = readSplitHoldRecord(account, lineId);
    const same = current !== null && current.attempt === read.attempt && (read.attempt !== null || current.at === read.at);
    if (!same) return { kind: "replaced", current: current && splitHoldKeeps(current, Date.now()) ? current : null };
    clearSplitHold(account, lineId, read.attempt);
    return { kind: "cleared" };
  } finally {
    release();
  }
}
