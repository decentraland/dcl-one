// The hold on a split sale whose outcome is unknown (Credits and MANA paying one item together), shared by every surface
// that sends one: the web Shop's dialog (detail/components/CombinedBuy.tsx) and the in-world Shop. It is kept per account
// and item, whatever listing the sale went through, in the storage every tab of this browser shares, so it outlives a tab
// and a restart, and a second sale of the same item waits on it whichever surface sent the first and whichever listing
// the second would go through.
//
// One run at a time per account and line: `claimSplitHold` takes the line's Web Lock (only if free) and reads the record
// inside it. The run holding the claim writes the record before the sale may leave (`writeSplitHold`: a browser that
// cannot keep it does not send), with the sale's terms (`SplitSale`), rewrites it with what it learns, and clears only its
// own attempt's (`clearSplitHold`). A record whose credit is known stays until an execution attributable to those terms,
// or the node's record of that credit, says what became of it (`settleSplitHold`, then `clearSettledSplitHold`): time
// alone never lets it go. Only a record from before credits were kept with it (no credit) is let go once `SPLIT_HOLD_MS`
// has passed.

import type { ShopClient } from "@shop/api/shop";
import { fetchUserPurchases, type PurchaseRecord } from "@shop/credits/credits";
import { assetKey, type AssetId } from "@shop/model/asset";
import type { ChainReader } from "../chain/rpc";
import { newAttemptId, webLock } from "../lib/web-lock";
import { isSplitSale, splitExecution, type SplitEvidenceReader, type SplitSale } from "./splitEvidence";

const KEY = "dcl-shop-buynow-split:v1:";
/** How long a record from before credits were kept with it holds; and how long Check again waits on a fresh one. */
export const SPLIT_HOLD_MS = 15 * 60 * 1000;
/** The node's purchase history, read this many lines at a time (its most) while looking for one credit. */
const HISTORY_PAGE = 200;
/** How many pages the lookup reads at most for one credit: well past any buyer's history (10 000 lines). */
export const HISTORY_MAX_PAGES = 50;

/**
 * A purchase history that couldn't be read to the credit or to its end: too many pages, or a server that sends the same
 * page again whatever offset it is asked for. Like a failed read, it says nothing: never "not listed", the hold stays.
 */
export class HistoryUnfinishedError extends Error {
  constructor() {
    super("The purchase history couldn't be read to its end.");
    this.name = "HistoryUnfinishedError";
  }
}
/**
 * How long a credit the node reports EXPIRED (unspent) must stay so, from when that was first seen, before its split is
 * taken as never gone through: past the index's lag, an expired credit can't be spent. As the in-world Shop waits.
 */
export const SPLIT_EXPIRY_CONFIRM_MS = 15 * 60 * 1000;

/**
 * A split sent and not settled. `attempt`: the run that wrote it, the only one that changes or clears it. `txHash` and
 * `chainId`: the sale's transaction, once one is known. `creditId`: the credit the split reserved. `creditExpiresAt` (unix
 * seconds): when that credit stops being spendable. `sale`: its terms, what an
 * execution must carry to count as this split (none on a record from before they were kept: nothing settles it then).
 * `expiredSeenAt`: when the node was first seen to report its credit EXPIRED.
 */
export type SplitHold = {
  at: number;
  attempt: string | null;
  txHash: string | null;
  chainId: number | null;
  creditId: string | null;
  creditExpiresAt: number | null;
  sale?: SplitSale | null;
  expiredSeenAt?: number | null;
};

/**
 * What a hold is kept under for one account: the item (`assetKey`), never its listing. The web Shop and the in-world Shop
 * may buy the same item through different listings (a creator's trade, the store's mint); either way it is the same item,
 * and one unsettled split of it holds every other.
 */
export function splitHoldLine(asset: AssetId): string {
  return assetKey(asset);
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
    return raw ? parseSplitHold(raw) : null;
  } catch {
    return null;
  }
}

/** A stored record read: null when it isn't one (not a hold's shape); throws when it isn't JSON. */
function parseSplitHold(raw: string): SplitHold | null {
  const parsed = JSON.parse(raw) as unknown;
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
    sale: isSplitSale(hold.sale) ? hold.sale : null,
    expiredSeenAt: typeof hold.expiredSeenAt === "number" ? hold.expiredSeenAt : null,
  };
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
 * Every hold of `account`, read strictly: throws where the storage can't be reached, or where a record that is there
 * can't be read or isn't a hold's, so a caller deciding whether another split may spend the allowance holds instead of
 * reading nothing. A key gone between listing and reading is simply gone.
 */
export function readAccountSplitHolds(account: string): { line: string; hold: SplitHold }[] {
  const storage = holdStorage();
  if (!storage) throw new Error("This browser's storage can't be read.");
  const prefix = splitHoldKey(account, "");
  const keys: string[] = [];
  for (let i = 0; i < storage.length; i++) {
    const key = storage.key(i);
    if (key && key.startsWith(prefix) && key.length > prefix.length) keys.push(key);
  }
  return keys.flatMap((key) => {
    const raw = storage.getItem(key);
    if (raw === null) return [];
    const hold = parseSplitHold(raw);
    if (!hold) throw new Error("A purchase's record in this browser can't be read.");
    return [{ line: key.slice(prefix.length), hold }];
  });
}

/**
 * The splits of `account` that may still spend its MANA allowance, on any surface (readAccountSplitHolds, so it throws
 * where it can't read them all). An exact approval sets the allowance to one gap, so another split's approval would take
 * an earlier one's, and another split finding the allowance covered could spend it: while one is listed, a split of
 * another item doesn't start. A run still preparing (no credit yet, a fresh hold) is listed; one with a credit is listed
 * until `over` proves that credit can't spend anything any more (its purchase over, or its expiry reached by the chain's
 * own time: splitCreditOverOnChain). An unknown expiry, or a local clock past it, proves nothing.
 */
export function splitsInFlight(account: string, now: number, over: (line: string, hold: SplitHold) => boolean): { line: string; hold: SplitHold }[] {
  return readAccountSplitHolds(account).filter(({ line, hold }) => (hold.creditId === null ? splitHoldFresh(hold, now) : !over(line, hold)));
}

/**
 * Whether `hold`'s credit provably can't be spent any more: the chain's finalized block time (`finalizedTime`) has
 * reached its expiry, and the manager refuses a credit once `block.timestamp >= expiresAt`, in every block after. The local
 * clock (`now`) only says when that is worth asking. An unknown expiry or chain, or a read that fails: not proven.
 */
export async function splitCreditOverOnChain(hold: SplitHold, finalizedTime: (chainId: number) => Promise<number>, now: number = Date.now()): Promise<boolean> {
  if (hold.creditId === null) return false;
  return creditExpiredOnChain(hold.creditExpiresAt, hold.chainId, finalizedTime, now);
}

/**
 * Whether a credit expiring at `expiresAt` (unix seconds) on `chainId` provably can't be spent any more: the chain's
 * finalized block time has reached it (the manager refuses a credit once `block.timestamp >= expiresAt`). The local clock
 * (`now`) only says when that is worth asking; an unknown expiry or chain, or a read that fails, proves nothing.
 */
export async function creditExpiredOnChain(expiresAt: number | null | undefined, chainId: number | null, finalizedTime: (chainId: number) => Promise<number>, now: number = Date.now()): Promise<boolean> {
  if (expiresAt == null || chainId === null || now < expiresAt * 1000) return false;
  try {
    return (await finalizedTime(chainId)) >= expiresAt;
  } catch {
    return false;
  }
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

/**
 * Clears the record only while it is `attempt`'s: an older attempt's outcome never wipes a newer one's record. Whether no
 * record of `attempt` is left (read back): false when the storage kept it (a removal that failed, or didn't stick).
 */
export function clearSplitHold(account: string | null, lineId: string, attempt: string | null): boolean {
  try {
    if (readSplitHoldRecord(account, lineId)?.attempt !== attempt) return true;
    holdStorage()?.removeItem(splitHoldKey(account, lineId));
  } catch {
    // Read back below.
  }
  return readSplitHoldRecord(account, lineId)?.attempt !== attempt;
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

/**
 * What became of a held split. `spent`: an execution of exactly its terms mined (complete it). `released`: the node reported
 * its credit EXPIRED, unspent, for the whole grace. `elsewhere`: the node reports its credit SETTLED by an execution that is provably not this split (nothing
 * to complete, and its credit is gone). `expiring`: the node reports it EXPIRED, not yet for the whole grace (the caller
 * notes when that was first seen, `noteSplitHoldExpiry`).
 */
export type SplitSettlement =
  | { kind: "spent"; txHash: string }
  | { kind: "released" }
  | { kind: "elsewhere" }
  | { kind: "expiring" };

/** A chain reader as attribution reads it: one without `transaction` attributes nothing. */
function evidenceOf(reader: ChainReader): SplitEvidenceReader {
  return {
    transaction: (chainId, hash) => (reader.transaction ? reader.transaction(chainId, hash) : Promise.resolve(null)),
    receipt: (chainId, hash) => reader.receipt(chainId, hash),
  };
}

/**
 * The node's record of one credit (listed by its lines, `<salt>:<position>`), read page by page until found or the end:
 * null once the whole history is read without it. One that can't be read to its end (`HISTORY_MAX_PAGES`, or a page that
 * starts where the last one did) throws `HistoryUnfinishedError`, as a failed read does.
 */
export async function findCreditRecord(account: string, creditId: string, client: ShopClient): Promise<PurchaseRecord | null> {
  const salt = creditId.toLowerCase();
  let head: string | null = null;
  for (let skip = 0, page = 0; ; page++) {
    if (page >= HISTORY_MAX_PAGES) throw new HistoryUnfinishedError();
    const { items, total, returned } = await fetchUserPurchases(account, { all: true, first: HISTORY_PAGE, skip }, client);
    const found = items.find((p) => p.id.split(":")[0]!.toLowerCase() === salt);
    if (found) return found;
    // The same first line again: the server ignored the offset, and reading on would read the same page forever.
    const first = items[0]?.id ?? null;
    if (first !== null && first === head) throw new HistoryUnfinishedError();
    head = first;
    // By the rows the server sent: the ones the decoder drops (a status it does not know) still take their place, and a
    // server that sends fewer than asked starts its next page right after them.
    skip += returned;
    if (returned === 0 || skip >= total) return null;
  }
}

/**
 * What became of a held split, decided only by its terms (the same rules the in-world Shop follows):
 * - its own transaction, read and matched (`splitExecution`): bought completes it;
 * - else the node's record of its credit: SETTLED by an execution of exactly its terms completes it, by the buyer's own
 *   call of that credit on other terms is `elsewhere`; EXPIRED, once the credit's own expiry has passed here too, releases
 *   it once that has been seen for `SPLIT_EXPIRY_CONFIRM_MS` (`expiring` before);
 * - its own dispatched bytes mined and reverted settle nothing on their own: that invocation failed, but a sibling
 *   invocation of the same terms may have run (the index may not show it yet), and a credit the node retired stays
 *   spendable on chain until it expires. So it is held, as anything else is, until the node's record says one of the above;
 * - anything else (no transaction, one this page can't read, a failed read, a credit still reserved or not listed): null,
 *   still held, however long ago it was sent.
 * A record with a credit and no terms is never settled. One from before credits were kept with it is let go once its hold
 * has run out.
 */
export async function settleSplitHold(
  hold: SplitHold,
  deps: { account: string | null; reader: ChainReader; client: ShopClient; now?: number; sleep?: (ms: number) => Promise<void> },
): Promise<SplitSettlement | null> {
  const now = deps.now ?? Date.now();
  if (!hold.creditId) return splitHoldFresh(hold, now) ? null : { kind: "released" };
  const sale = hold.sale;
  if (!sale) return null;
  const evidence = evidenceOf(deps.reader);
  if (hold.txHash) {
    const own = await splitExecution(evidence, sale, hold.txHash, { sleep: deps.sleep });
    if (own === "bought") return { kind: "spent", txHash: hold.txHash };
    // Its own bytes reverted, another execution, none yet, or one this page can't read: the node's record decides.
  }
  if (!deps.account) return null;
  let credit: PurchaseRecord | null;
  try {
    credit = await findCreditRecord(deps.account, hold.creditId, deps.client);
  } catch {
    return null;
  }
  if (credit?.status === "SETTLED") {
    if (!credit.txHash) return null;
    const spent = await splitExecution(evidence, sale, credit.txHash, { sleep: deps.sleep });
    if (spent === "bought") return { kind: "spent", txHash: credit.txHash };
    if (spent === "other") return { kind: "elsewhere" };
    return null;
  }
  if (credit?.status === "EXPIRED") {
    // The node lists a credit EXPIRED only once the chain, as far as it has read it, is past its expiry. This browser waits
    // for that expiry too, a second safeguard: with no expiry known to check, or before it, there is no answer, and the
    // grace counts only from a sighting made past it (`expirySeenAt`).
    if (hold.creditExpiresAt === null || now < hold.creditExpiresAt * 1000) return null;
    const seen = expirySeenAt(hold);
    return seen !== null && now >= seen && now - seen >= SPLIT_EXPIRY_CONFIRM_MS ? { kind: "released" } : { kind: "expiring" };
  }
  return null;
}

/**
 * When its credit was first seen EXPIRED past that credit's own expiry: the only sighting the grace may count from. One
 * made before the expiry (or with no expiry known) is none, and the next sighting past it takes its place.
 */
function expirySeenAt(hold: SplitHold): number | null {
  const seen = hold.expiredSeenAt ?? null;
  return seen !== null && hold.creditExpiresAt !== null && seen >= hold.creditExpiresAt * 1000 ? seen : null;
}

/**
 * Notes, under the line's lock and only on the attempt that was read, when its credit was first seen EXPIRED past its own
 * expiry: the grace runs from then, in every tab and surface, and survives a restart. A sighting kept from before that
 * expiry is replaced by this one. `noted` (or already), `replaced` (another attempt's record is there now, or none), or
 * `busy` (a run holds the line's lock, or there are no Web Locks).
 */
export async function noteSplitHoldExpiry(
  locks: LockManager | null | undefined,
  account: string,
  lineId: string,
  read: SplitHold,
  now: number = Date.now(),
): Promise<"noted" | "replaced" | "busy"> {
  if (expirySeenAt(read) !== null) return "noted";
  const release = await webLock(locks, splitHoldLockName(account, lineId));
  if (!release) return "busy";
  try {
    const current = readSplitHoldRecord(account, lineId);
    if (!current || current.attempt !== read.attempt || (read.attempt === null && current.at !== read.at)) return "replaced";
    if (expirySeenAt(current) !== null) return "noted";
    return writeSplitHold(account, lineId, { ...current, expiredSeenAt: now }) ? "noted" : "busy";
  } finally {
    release();
  }
}

/**
 * Applying an outcome settled for `read`. `cleared`: it was still the line's record (the same attempt; for one with no
 * attempt, the same `at`) and is gone, read back. `replaced`: another run settled it and wrote another meanwhile
 * (`current`, kept, or null): the outcome is not applied to that one. `busy`: a run holds the line's lock (or there are no
 * Web Locks), or the storage kept the record when it was removed (`unremoved`: it is still there); try later.
 */
export type SplitHoldClearing = { kind: "cleared" } | { kind: "replaced"; current: SplitHold | null } | { kind: "busy"; unremoved?: true };

/** Clears the settled `read` under the line's lock, only while it is still the record; see `SplitHoldClearing`. */
export async function clearSettledSplitHold(locks: LockManager | null | undefined, account: string, lineId: string, read: SplitHold): Promise<SplitHoldClearing> {
  const release = await webLock(locks, splitHoldLockName(account, lineId));
  if (!release) return { kind: "busy" };
  try {
    const current = readSplitHoldRecord(account, lineId);
    const same = current !== null && current.attempt === read.attempt && (read.attempt !== null || current.at === read.at);
    if (!same) return { kind: "replaced", current: current && splitHoldKeeps(current, Date.now()) ? current : null };
    return clearSplitHold(account, lineId, read.attempt) ? { kind: "cleared" } : { kind: "busy", unremoved: true };
  } finally {
    release();
  }
}
