// The split sale hold every surface shares: one claim per account and line at a time, the record read inside the claim,
// writes and clears scoped to their attempt, and what settles a hold.

import { afterEach, describe, expect, it, vi } from "vitest";

import { CHAIN } from "@shop/model/chain";
import { itemAsset } from "@shop/model/asset";
import { BOOK } from "@shop/payment/fixtures";
import { createFakeChain, createFakeLocks, createFakeShop, splitSaleTransaction } from "../fakes";
import {
  claimSplitHold,
  clearSettledSplitHold,
  clearSplitHold,
  findCreditRecord,
  HISTORY_MAX_PAGES,
  HistoryUnfinishedError,
  readSplitHold,
  readSplitHoldRecord,
  noteSplitHoldExpiry,
  readAccountSplitHolds,
  settleSplitHold,
  splitCreditOverOnChain,
  splitsInFlight,
  SPLIT_EXPIRY_CONFIRM_MS,
  SPLIT_HOLD_MS,
  splitHoldKey,
  splitHoldLine,
  splitHoldLockName,
  writeSplitHold,
  readerFinalizedTime,
  type SplitHold,
} from "./splitHold";
import { splitEnvelope, type SplitSale } from "./splitEvidence";

afterEach(() => {
  localStorage.clear();
  vi.restoreAllMocks();
});

const ACCOUNT = BOOK.buyer;
const ASSET = itemAsset(CHAIN.POLYGON, BOOK.collection, "7");
const LINE = splitHoldLine(ASSET);
const CREDIT = `0x${"c1".repeat(32)}`;
const seconds = () => Math.floor(Date.now() / 1000);
/** A split's terms, as the web Shop's dialog or the in-world Shop keeps them with its hold. */
const SALE: SplitSale = {
  buyer: ACCOUNT.toLowerCase(),
  chainId: CHAIN.POLYGON,
  manager: "0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5",
  creditId: CREDIT,
  creditWei: "1000000000000000000",
  gapWei: "1300000000000000000",
  target: BOOK.store.contract.toLowerCase(),
  selector: "0x12345678",
  callData: "0xabcdef",
};
const hold = (over: Partial<SplitHold> = {}): SplitHold => ({
  at: Date.now(),
  attempt: "a",
  txHash: null,
  chainId: CHAIN.POLYGON,
  creditId: CREDIT,
  creditExpiresAt: seconds() + 300,
  sale: SALE,
  expiredSeenAt: null,
  expiryProvenAt: null,
  ...over,
});

describe("what a hold is kept under", () => {
  it("the item, whatever listing a surface buys it through: the same item is one hold, another item another", () => {
    // Its line names the item and nothing of a listing: the web's creator's trade and the in-world Shop's store mint of
    // the same item are one hold.
    const line = splitHoldLine(ASSET);
    expect(line).toBe(`${CHAIN.POLYGON}:${BOOK.collection}:item:7`);
    expect(splitHoldLine(itemAsset(CHAIN.POLYGON, BOOK.collection, "8"))).not.toBe(line);
    writeSplitHold(ACCOUNT, line, hold());
    expect(readSplitHold(ACCOUNT, splitHoldLine(itemAsset(CHAIN.POLYGON, BOOK.collection, "7")))).not.toBeNull();
    // The key and the lock name lowercase the account, so a checksummed address finds the same hold.
    expect(splitHoldKey(ACCOUNT.toUpperCase().replace("0X", "0x"), LINE)).toBe(splitHoldKey(ACCOUNT, LINE));
    expect(splitHoldLockName(ACCOUNT.toUpperCase().replace("0X", "0x"), LINE)).toBe(splitHoldLockName(ACCOUNT, LINE));
  });
});

describe("an account's splits that may still spend its allowance", () => {
  const OTHER = splitHoldLine(itemAsset(CHAIN.POLYGON, BOOK.collection, "8"));

  it("reads every hold of the account strictly: none to read, or one it can't read or understand, throws", () => {
    expect(writeSplitHold(ACCOUNT, LINE, hold())).toBe(true);
    expect(writeSplitHold(`0x${"99".repeat(20)}`, OTHER, hold())).toBe(true);
    expect(readAccountSplitHolds(ACCOUNT).map((h) => h.line)).toEqual([LINE]);
    localStorage.setItem(splitHoldKey(ACCOUNT, OTHER), "{not json");
    expect(() => readAccountSplitHolds(ACCOUNT)).toThrow();
    localStorage.setItem(splitHoldKey(ACCOUNT, OTHER), JSON.stringify({ not: "a hold" }));
    expect(() => readAccountSplitHolds(ACCOUNT)).toThrow();
    localStorage.removeItem(splitHoldKey(ACCOUNT, OTHER));
    const getItem = Storage.prototype.getItem;
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(function (this: Storage, key: string) {
      if (key === splitHoldKey(ACCOUNT, LINE)) throw new Error("read failed");
      return getItem.call(this, key);
    });
    expect(() => readAccountSplitHolds(ACCOUNT)).toThrow();
  });

  it("rejects a field of the wrong type strictly, where the tolerant read takes it as absent, and reads every older format", () => {
    for (const bad of [{ creditId: 12345 }, { chainId: "137" }, { creditExpiresAt: "1700000000" }, { attempt: 7 }, { sale: { buyer: "x" } }, { expiredSeenAt: "soon" }, { expiryProvenAt: "soon" }]) {
      localStorage.setItem(splitHoldKey(ACCOUNT, OTHER), JSON.stringify({ ...hold(), ...bad }));
      expect(() => readAccountSplitHolds(ACCOUNT)).toThrow();
      expect(readSplitHoldRecord(ACCOUNT, OTHER)).not.toBeNull();
    }
    // A bare timestamp (the first format), and a record from before the credit, its terms or the expiry note were kept.
    localStorage.setItem(splitHoldKey(ACCOUNT, OTHER), JSON.stringify(Date.now()));
    localStorage.setItem(splitHoldKey(ACCOUNT, LINE), JSON.stringify({ at: Date.now(), attempt: null, txHash: null }));
    expect(readAccountSplitHolds(ACCOUNT).map((h) => h.line).sort()).toEqual([LINE, OTHER].sort());
  });

  it("throws without storage, and skips a key gone between listing and reading", () => {
    expect(writeSplitHold(ACCOUNT, LINE, hold())).toBe(true);
    const getItem = Storage.prototype.getItem;
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(function (this: Storage, key: string) {
      return key === splitHoldKey(ACCOUNT, LINE) ? null : getItem.call(this, key);
    });
    expect(readAccountSplitHolds(ACCOUNT)).toEqual([]);
    vi.restoreAllMocks();
    vi.stubGlobal("localStorage", undefined);
    try {
      expect(() => readAccountSplitHolds(ACCOUNT)).toThrow();
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("lists a hold with a credit until `over` proves it can't spend, and a run still preparing while fresh", () => {
    expect(writeSplitHold(ACCOUNT, LINE, hold({ creditExpiresAt: null }))).toBe(true);
    expect(writeSplitHold(ACCOUNT, OTHER, hold({ creditId: null, creditExpiresAt: null, sale: null }))).toBe(true);
    const now = Date.now();
    expect(splitsInFlight(ACCOUNT, now, () => false).map((h) => h.line).sort()).toEqual([LINE, OTHER].sort());
    expect(splitsInFlight(ACCOUNT, now, (line) => line === LINE).map((h) => h.line)).toEqual([OTHER]);
    expect(splitsInFlight(ACCOUNT, now + SPLIT_HOLD_MS, () => false).map((h) => h.line)).toEqual([LINE]);
  });

  it("proves a credit over only by the chain's finalized time reaching its expiry, asked once the local clock is past it", async () => {
    const asked: number[] = [];
    const time = (t: number) => async (chainId: number) => (asked.push(chainId), t);
    const expiring = hold({ creditExpiresAt: 1_000 });
    expect(await splitCreditOverOnChain(expiring, time(5_000), 999_000)).toBe(false);
    expect(asked).toEqual([]);
    expect(await splitCreditOverOnChain(expiring, time(999), 2_000_000)).toBe(false);
    expect(await splitCreditOverOnChain(expiring, time(1_000), 2_000_000)).toBe(true);
    expect(await splitCreditOverOnChain(expiring, async () => Promise.reject(new Error("rpc down")), 2_000_000)).toBe(false);
    expect(await splitCreditOverOnChain(hold({ creditExpiresAt: null }), time(5_000), 2_000_000)).toBe(false);
    expect(await splitCreditOverOnChain(hold({ chainId: null, creditExpiresAt: 1_000 }), time(5_000), 2_000_000)).toBe(false);
  });
});

describe("claiming a line", () => {
  it("two contenders at once: one claims it, the other finds it busy; released, it can be claimed again", async () => {
    const locks = createFakeLocks();
    const [first, second] = await Promise.all([claimSplitHold(locks, ACCOUNT, LINE), claimSplitHold(locks, ACCOUNT, LINE)]);
    const kinds = [first.kind, second.kind].sort();
    expect(kinds).toEqual(["busy", "claimed"]);
    const claimed = (first.kind === "claimed" ? first : second) as Extract<typeof first, { kind: "claimed" }>;
    expect(claimed.attempt).toMatch(/^[0-9a-f]{32}$/);
    claimed.release();
    // A lock is let go once its holder's callback settles, a turn later, as Web Locks do.
    await vi.waitFor(() => expect(locks.held.size).toBe(0));
    expect((await claimSplitHold(locks, ACCOUNT, LINE)).kind).toBe("claimed");
  });

  it("a hold left by another window or surface is read inside the claim: it is answered, and the lock let go", async () => {
    const locks = createFakeLocks();
    const left = hold({ attempt: "other" });
    expect(writeSplitHold(ACCOUNT, LINE, left)).toBe(true);
    const claim = await claimSplitHold(locks, ACCOUNT, LINE);
    expect(claim).toEqual({ kind: "held", hold: left });
    expect(locks.held.size).toBe(0);
  });

  it("without Web Locks nothing is claimed, and nothing proves a split goes out once", async () => {
    expect(await claimSplitHold(null, ACCOUNT, LINE)).toEqual({ kind: "unguarded" });
    expect(await claimSplitHold(undefined, ACCOUNT, LINE)).toEqual({ kind: "unguarded" });
    expect(await clearSettledSplitHold(null, ACCOUNT, LINE, hold())).toEqual({ kind: "busy" });
    expect(readSplitHoldRecord(ACCOUNT, LINE)).toBeNull();
  });
});

describe("the record of an attempt", () => {
  it("is never written over another attempt's that still holds the line; a record from before credits, run out, is", () => {
    expect(writeSplitHold(ACCOUNT, LINE, hold({ attempt: "a" }))).toBe(true);
    expect(writeSplitHold(ACCOUNT, LINE, hold({ attempt: "b" }))).toBe(false);
    expect(readSplitHold(ACCOUNT, LINE)?.attempt).toBe("a");
    // Its own attempt rewrites it (the hash it learned).
    expect(writeSplitHold(ACCOUNT, LINE, hold({ attempt: "a", txHash: "0xab" }))).toBe(true);
    localStorage.setItem(splitHoldKey(ACCOUNT, LINE), String(Date.now() - SPLIT_HOLD_MS - 1));
    expect(readSplitHold(ACCOUNT, LINE)).toBeNull();
    expect(writeSplitHold(ACCOUNT, LINE, hold({ attempt: "b" }))).toBe(true);
  });

  it("a browser that cannot keep it says so, so the sale is not sent", () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("The quota has been exceeded.", "QuotaExceededError");
    });
    expect(writeSplitHold(ACCOUNT, LINE, hold())).toBe(false);
  });

  it("with a credit, it holds however long ago it was sent; one from before credits, for the hold only", () => {
    writeSplitHold(ACCOUNT, LINE, hold({ at: Date.now() - 10 * SPLIT_HOLD_MS, txHash: "0xab" }));
    expect(readSplitHold(ACCOUNT, LINE)).not.toBeNull();
    localStorage.setItem(splitHoldKey(ACCOUNT, LINE), String(1_000));
    expect(readSplitHold(ACCOUNT, LINE, 1_000 + SPLIT_HOLD_MS - 1)).not.toBeNull();
    expect(readSplitHold(ACCOUNT, LINE, 1_000 + SPLIT_HOLD_MS)).toBeNull();
  });

  it("a stale attempt never wipes a newer one: not when it clears, not when its outcome is applied", async () => {
    const older = hold({ attempt: "older", creditExpiresAt: seconds() - 1 });
    writeSplitHold(ACCOUNT, LINE, older);
    // Another window settled the older one and sent a newer split of the line.
    localStorage.removeItem(splitHoldKey(ACCOUNT, LINE));
    const newer = hold({ attempt: "newer" });
    expect(writeSplitHold(ACCOUNT, LINE, newer)).toBe(true);
    clearSplitHold(ACCOUNT, LINE, "older");
    expect(readSplitHold(ACCOUNT, LINE)).toEqual(newer);
    expect(await clearSettledSplitHold(createFakeLocks(), ACCOUNT, LINE, older)).toEqual({ kind: "replaced", current: newer });
    expect(readSplitHold(ACCOUNT, LINE)).toEqual(newer);
    // Its own outcome clears it.
    expect(await clearSettledSplitHold(createFakeLocks(), ACCOUNT, LINE, newer)).toEqual({ kind: "cleared" });
    expect(readSplitHoldRecord(ACCOUNT, LINE)).toBeNull();
  });

  it("a removal the storage refuses is never reported cleared: the record stays, and once the storage recovers it clears", async () => {
    const read = hold({ attempt: "a" });
    writeSplitHold(ACCOUNT, LINE, read);
    const refused = vi.spyOn(Storage.prototype, "removeItem").mockImplementation(() => {
      throw new DOMException("The operation is insecure.", "SecurityError");
    });
    expect(await clearSettledSplitHold(createFakeLocks(), ACCOUNT, LINE, read)).toEqual({ kind: "busy", unremoved: true });
    expect(clearSplitHold(ACCOUNT, LINE, "a")).toBe(false);
    expect(readSplitHoldRecord(ACCOUNT, LINE)?.attempt).toBe("a");
    // One that keeps it without a word is caught too: the record is read back.
    refused.mockImplementation(() => {});
    expect(await clearSettledSplitHold(createFakeLocks(), ACCOUNT, LINE, read)).toEqual({ kind: "busy", unremoved: true });
    refused.mockRestore();
    expect(await clearSettledSplitHold(createFakeLocks(), ACCOUNT, LINE, read)).toEqual({ kind: "cleared" });
    expect(readSplitHoldRecord(ACCOUNT, LINE)).toBeNull();
  });

  it("an outcome waits while another run holds the line", async () => {
    const locks = createFakeLocks();
    writeSplitHold(ACCOUNT, LINE, hold());
    locks.held.add(splitHoldLockName(ACCOUNT, LINE));
    expect(await clearSettledSplitHold(locks, ACCOUNT, LINE, hold())).toEqual({ kind: "busy" });
    expect(readSplitHoldRecord(ACCOUNT, LINE)).not.toBeNull();
  });
});

describe("reading a credit's record from the history", () => {
  /** A full page of other credits' lines, `page` apart from every other page. */
  const otherLines = (page: number) =>
    Array.from({ length: 200 }, (_, n) => ({ id: `0x${(page * 200 + n + 1).toString(16).padStart(64, "0")}:0`, status: "SETTLED", usdCents: 10, createdAt: 1, txHash: null }));

  it("a history that never ends (full pages, no total) is read at most HISTORY_MAX_PAGES pages, then is no answer: held", async () => {
    let calls = 0;
    // A server that sends no total; it runs dry only well past the cap, so a lookup without one shows here as more calls.
    const shop = createFakeShop({ userPurchases: () => ({ purchases: ++calls > HISTORY_MAX_PAGES + 10 ? [] : otherLines(calls) }) });
    await expect(findCreditRecord(ACCOUNT, CREDIT, shop.client)).rejects.toBeInstanceOf(HistoryUnfinishedError);
    expect(calls).toBe(HISTORY_MAX_PAGES);
    expect(await settleSplitHold(hold({ creditExpiresAt: seconds() - 1 }), { account: ACCOUNT, reader: createFakeChain(), client: shop.client })).toBeNull();
  });

  it("full pages none of whose rows decode, with no total, are read up to the cap only: no answer, held", async () => {
    let calls = 0;
    // Every row has a status the decoder drops: no line to compare, so only the cap stops it.
    const undecodable = (page: number) => otherLines(page).map((line) => ({ ...line, status: "SOMETHING_NEW" }));
    const shop = createFakeShop({ userPurchases: () => ({ purchases: ++calls > HISTORY_MAX_PAGES + 10 ? [] : undecodable(calls) }) });
    await expect(findCreditRecord(ACCOUNT, CREDIT, shop.client)).rejects.toBeInstanceOf(HistoryUnfinishedError);
    expect(calls).toBe(HISTORY_MAX_PAGES);
  });

  it("a server that ignores the offset (the same page every time) is caught on the second page: no answer, held", async () => {
    let calls = 0;
    const shop = createFakeShop({ userPurchases: () => ((calls += 1), { purchases: otherLines(0) }) });
    await expect(findCreditRecord(ACCOUNT, CREDIT, shop.client)).rejects.toBeInstanceOf(HistoryUnfinishedError);
    expect(calls).toBe(2);
  });

  it("control: a history read to its end without the credit is null; with it, its record", async () => {
    const lines = [...otherLines(0), { id: `${CREDIT}:0`, status: "EXPIRED", usdCents: 50, createdAt: 1, txHash: null }];
    const listed = createFakeShop({ userPurchases: ({ query }: { query: URLSearchParams }) => ({ purchases: lines.slice(Number(query.get("offset") ?? 0)).slice(0, 200), total: lines.length }) });
    expect(await findCreditRecord(ACCOUNT, CREDIT, listed.client)).toMatchObject({ status: "EXPIRED" });
    const absent = createFakeShop({ userPurchases: () => ({ purchases: otherLines(0).slice(0, 10), total: 10 }) });
    expect(await findCreditRecord(ACCOUNT, CREDIT, absent.client)).toBeNull();
  });
});

describe("what settles a hold", () => {
  const HASH = `0x${"5a".repeat(32)}`;
  const SETTLED_BY = `0x${"ab".repeat(32)}`;
  function deps(listed: { status: "PENDING" | "SETTLED" | "EXPIRED"; txHash?: string } | null) {
    const chain = createFakeChain();
    const shop = createFakeShop({
      userPurchases: () => ({
        purchases: listed ? [{ id: `${CREDIT}:0`, status: listed.status, usdCents: 50, createdAt: 1, txHash: listed.txHash ?? null }] : [],
        total: listed ? 1 : 0,
      }),
    });
    return { chain, shop, deps: { account: ACCOUNT, reader: chain, client: shop.client } };
  }
  const mined = (chain: ReturnType<typeof createFakeChain>, hash: string, tx: ReturnType<typeof splitSaleTransaction>, status: "success" | "reverted" = "success") => {
    chain.transactions.set(hash.toLowerCase(), tx);
    chain.receipts.set(hash.toLowerCase(), status);
  };

  /** The sale as dispatched in `tx`: its terms and the identity of those bytes. */
  const dispatchedAs = (tx: ReturnType<typeof splitSaleTransaction>): SplitSale => ({ ...SALE, envelope: splitEnvelope(tx.input) });
  const ANOTHER_SIGNATURE = `0x${"22".repeat(65)}` as const;

  it("its own transaction, carrying exactly its terms, mined completes it; its own bytes reverted settle nothing: the node's record decides", async () => {
    const ok = deps(null);
    mined(ok.chain, HASH, splitSaleTransaction(SALE));
    expect(await settleSplitHold(hold({ txHash: HASH }), ok.deps)).toEqual({ kind: "spent", txHash: HASH });
    const tx = splitSaleTransaction(SALE);
    // Still reserved, or not listed (the index may be behind a sibling that ran): held, its credit not given back.
    for (const listed of [null, { status: "PENDING" as const }]) {
      const reverted = deps(listed);
      mined(reverted.chain, HASH, tx, "reverted");
      expect(await settleSplitHold(hold({ txHash: HASH, sale: dispatchedAs(tx) }), reverted.deps), JSON.stringify(listed)).toBeNull();
    }
    // Expired unspent, its credit past its expiry (twenty minutes ago): let go only past the grace, as any other.
    const expired = deps({ status: "EXPIRED" });
    mined(expired.chain, HASH, tx, "reverted");
    const over = { creditExpiresAt: seconds() - 20 * 60 };
    expect(await settleSplitHold(hold({ ...over, txHash: HASH, sale: dispatchedAs(tx) }), expired.deps)).toEqual({ kind: "expiring" });
    const seen = Date.now() - SPLIT_EXPIRY_CONFIRM_MS;
    expect(await settleSplitHold(hold({ ...over, txHash: HASH, sale: dispatchedAs(tx), expiryProvenAt: seen }), expired.deps)).toEqual({ kind: "released" });
  });

  it("a revert of its terms in another envelope (signed again) is not its own: the node's record decides, and nothing is released on it", async () => {
    const mine = splitSaleTransaction(SALE);
    for (const listed of [null, { status: "PENDING" as const }]) {
      const other = deps(listed);
      mined(other.chain, HASH, splitSaleTransaction(SALE, { signature: ANOTHER_SIGNATURE }), "reverted");
      expect(await settleSplitHold(hold({ txHash: HASH, sale: dispatchedAs(mine) }), other.deps)).toBeNull();
    }
    // Nor with no envelope kept at all.
    const none = deps(null);
    mined(none.chain, HASH, mine, "reverted");
    expect(await settleSplitHold(hold({ txHash: HASH }), none.deps)).toBeNull();
  });

  it("its own bytes reverted, then the same execution ran in another transaction the node settled: completed, not released", async () => {
    const tx = splitSaleTransaction(SALE);
    const replay = deps({ status: "SETTLED", txHash: SETTLED_BY });
    mined(replay.chain, HASH, tx, "reverted");
    mined(replay.chain, SETTLED_BY, tx);
    expect(await settleSplitHold(hold({ txHash: HASH, sale: dispatchedAs(tx) }), replay.deps)).toEqual({ kind: "spent", txHash: SETTLED_BY });
    // Its credit SETTLED by another purchase: spent elsewhere. SETTLED by nothing it can read: held.
    const elsewhere = deps({ status: "SETTLED", txHash: SETTLED_BY });
    mined(elsewhere.chain, HASH, tx, "reverted");
    mined(elsewhere.chain, SETTLED_BY, splitSaleTransaction({ ...SALE, callData: "0x0bad" }));
    expect(await settleSplitHold(hold({ txHash: HASH, sale: dispatchedAs(tx) }), elsewhere.deps)).toEqual({ kind: "elsewhere" });
    const unread = deps({ status: "SETTLED", txHash: SETTLED_BY });
    mined(unread.chain, HASH, tx, "reverted");
    expect(await settleSplitHold(hold({ txHash: HASH, sale: dispatchedAs(tx) }), unread.deps)).toBeNull();
  });

  it("its own bytes reverted and the node's record can't be read: held, never released on the revert alone", async () => {
    const chain = createFakeChain();
    const shop = createFakeShop({
      userPurchases: () => {
        throw new Error("upstream unavailable");
      },
    });
    const tx = splitSaleTransaction(SALE);
    mined(chain, HASH, tx, "reverted");
    expect(await settleSplitHold(hold({ txHash: HASH, sale: dispatchedAs(tx) }), { account: ACCOUNT, reader: chain, client: shop.client })).toBeNull();
  });

  it("a successful receipt for a transaction that is not this split's (another buyer's, or none to read) completes nothing", async () => {
    const foreign = deps(null);
    mined(foreign.chain, HASH, splitSaleTransaction(SALE, { user: `0x${"cc".repeat(20)}` }));
    expect(await settleSplitHold(hold({ txHash: HASH }), foreign.deps)).toBeNull();
    const unread = deps(null);
    unread.chain.receipts.set(HASH, "success");
    expect(await settleSplitHold(hold({ txHash: HASH }), { ...unread.deps, sleep: async () => {} })).toBeNull();
  });

  it.each([
    ["reverted", "reverted" as const],
    ["not mined yet", null],
  ])("its credit SETTLED in the buyer's own call on other terms, %s, is no proof it was spent elsewhere: held", async (_, status) => {
    const { chain, deps: d } = deps({ status: "SETTLED", txHash: SETTLED_BY });
    chain.transactions.set(SETTLED_BY.toLowerCase(), splitSaleTransaction({ ...SALE, callData: "0x0bad" }));
    if (status) chain.receipts.set(SETTLED_BY.toLowerCase(), status);
    expect(await settleSplitHold(hold(), { ...d, sleep: async () => {} })).toBeNull();
  });

  it("then the node's record of its credit: SETTLED by this split completes it, by another purchase is spent elsewhere, unread holds", async () => {
    const own = deps({ status: "SETTLED", txHash: SETTLED_BY });
    mined(own.chain, SETTLED_BY, splitSaleTransaction(SALE));
    expect(await settleSplitHold(hold({ txHash: HASH }), own.deps)).toEqual({ kind: "spent", txHash: SETTLED_BY });
    const elsewhere = deps({ status: "SETTLED", txHash: SETTLED_BY });
    mined(elsewhere.chain, SETTLED_BY, splitSaleTransaction({ ...SALE, callData: "0x0bad" }));
    expect(await settleSplitHold(hold(), elsewhere.deps)).toEqual({ kind: "elsewhere" });
    // SETTLED alone, with nothing on chain to read: the salt says the credit was spent, not on what.
    expect(await settleSplitHold(hold(), deps({ status: "SETTLED", txHash: SETTLED_BY }).deps)).toBeNull();
    expect(await settleSplitHold(hold(), deps({ status: "PENDING" }).deps)).toBeNull();
    expect(await settleSplitHold(hold(), deps(null).deps)).toBeNull();
  });

  it("EXPIRED, its credit past its expiry, lets it go only once seen so for the whole grace: before, it is expiring", async () => {
    const { deps: d } = deps({ status: "EXPIRED" });
    // Its credit expired twenty minutes ago: every sighting below is past it.
    const over = { creditExpiresAt: seconds() - 20 * 60 };
    expect(await settleSplitHold(hold(over), d)).toEqual({ kind: "expiring" });
    const seen = Date.now() - SPLIT_EXPIRY_CONFIRM_MS + 1_000;
    expect(await settleSplitHold(hold({ ...over, expiryProvenAt: seen }), d)).toEqual({ kind: "expiring" });
    expect(await settleSplitHold(hold({ ...over, expiryProvenAt: seen }), { ...d, now: seen + SPLIT_EXPIRY_CONFIRM_MS })).toEqual({ kind: "released" });
  });

  it("EXPIRED while its credit can still be spent here is no answer, however long seen so: held, the grace not started", async () => {
    const { deps: d } = deps({ status: "EXPIRED" });
    const live = { creditExpiresAt: seconds() + 300 };
    expect(await settleSplitHold(hold(live), d)).toBeNull();
    const long = Date.now() - 10 * SPLIT_EXPIRY_CONFIRM_MS;
    expect(await settleSplitHold(hold({ ...live, expiryProvenAt: long }), d)).toBeNull();
  });

  it("a record with a credit and no terms is never settled, whatever its receipt says", async () => {
    const { chain, deps: d } = deps({ status: "EXPIRED" });
    mined(chain, HASH, splitSaleTransaction(SALE));
    expect(await settleSplitHold(hold({ txHash: HASH, sale: null, expiryProvenAt: 1 }), d)).toBeNull();
  });

  it("when its credit was first seen EXPIRED is noted on the attempt that was read, under the line's lock", async () => {
    const locks = createFakeLocks();
    // Its credit expired at 1 s: the sightings below, at 5 s and 9 s, are past it.
    const read = hold({ attempt: "a", creditExpiresAt: 1 });
    writeSplitHold(ACCOUNT, LINE, read);
    expect(await noteSplitHoldExpiry(locks, ACCOUNT, LINE, read, 5_000)).toBe("noted");
    expect(readSplitHoldRecord(ACCOUNT, LINE)?.expiryProvenAt).toBe(5_000);
    // Noted once: a later look does not move it.
    expect(await noteSplitHoldExpiry(locks, ACCOUNT, LINE, read, 9_000)).toBe("noted");
    expect(readSplitHoldRecord(ACCOUNT, LINE)?.expiryProvenAt).toBe(5_000);
    expect(await noteSplitHoldExpiry(locks, ACCOUNT, LINE, hold({ attempt: "older", creditExpiresAt: 1 }))).toBe("replaced");
    locks.held.add(splitHoldLockName(ACCOUNT, LINE));
    expect(await noteSplitHoldExpiry(locks, ACCOUNT, LINE, hold({ attempt: "a", creditExpiresAt: 1 }))).toBe("busy");
  });

  it("a sighting from before its credit's expiry doesn't start the grace: held until expiry, then the first sighting past it counts", async () => {
    const { chain, deps: d } = deps({ status: "EXPIRED" });
    const expiry = (seconds() + 60) * 1000;
    // The chain's finalized time has reached the expiry: only the local sightings are early or not.
    chain.chainTime = expiry / 1000;
    // EXPIRED was noted a minute before the credit's own expiry.
    const early = hold({ attempt: "a", creditExpiresAt: expiry / 1000, expiryProvenAt: expiry - 60_000 });
    writeSplitHold(ACCOUNT, LINE, early);
    expect(await settleSplitHold(early, { ...d, now: expiry - 1 })).toBeNull();
    // Just past its expiry: expiring, not released on the early sighting.
    expect(await settleSplitHold(early, { ...d, now: expiry + 1 })).toEqual({ kind: "expiring" });
    // That look is noted in its place, under the attempt's lock, and the grace runs from it.
    expect(await noteSplitHoldExpiry(createFakeLocks(), ACCOUNT, LINE, early, expiry + 1)).toBe("noted");
    const noted = readSplitHoldRecord(ACCOUNT, LINE)!;
    expect(noted.expiryProvenAt).toBe(expiry + 1);
    expect(await settleSplitHold(noted, { ...d, now: expiry + SPLIT_EXPIRY_CONFIRM_MS })).toEqual({ kind: "expiring" });
    expect(await settleSplitHold(noted, { ...d, now: expiry + 1 + SPLIT_EXPIRY_CONFIRM_MS })).toEqual({ kind: "released" });
  });

  it("EXPIRED for a credit whose expiry isn't known is no answer, however old the sighting: held", async () => {
    const { deps: d } = deps({ status: "EXPIRED" });
    expect(await settleSplitHold(hold({ creditExpiresAt: null, expiryProvenAt: 1 }), d)).toBeNull();
  });

  it("EXPIRED with its expiry past on this browser's clock but not yet at the chain's finalized block is no answer: held, however old the sighting", async () => {
    const { chain, deps: d } = deps({ status: "EXPIRED" });
    const expiresAt = seconds() - 20 * 60;
    // A clock running ahead: the finalized block is a second short of the credit's expiry, so the manager can still spend it.
    chain.chainTime = expiresAt - 1;
    expect(await settleSplitHold(hold({ creditExpiresAt: expiresAt }), d)).toBeNull();
    expect(await settleSplitHold(hold({ creditExpiresAt: expiresAt, expiryProvenAt: Date.now() - 10 * SPLIT_EXPIRY_CONFIRM_MS }), d)).toBeNull();
    // The finalized block reaches it: expiring, then released once seen so for the whole grace.
    chain.chainTime = expiresAt;
    expect(await settleSplitHold(hold({ creditExpiresAt: expiresAt }), d)).toEqual({ kind: "expiring" });
    expect(await settleSplitHold(hold({ creditExpiresAt: expiresAt, expiryProvenAt: Date.now() - SPLIT_EXPIRY_CONFIRM_MS }), d)).toEqual({ kind: "released" });
  });

  it("EXPIRED with no finalized time to read (a read that fails, or a reader without it) is no answer: held", async () => {
    const { chain, deps: d } = deps({ status: "EXPIRED" });
    const past = { creditExpiresAt: seconds() - 20 * 60, expiryProvenAt: Date.now() - 10 * SPLIT_EXPIRY_CONFIRM_MS };
    chain.finalizedTime = async () => {
      throw new Error("rate limited");
    };
    expect(await settleSplitHold(hold(past), d)).toBeNull();
    const { finalizedTime: _none, ...withoutTime } = chain;
    expect(await settleSplitHold(hold(past), { ...d, reader: withoutTime })).toBeNull();
  });

  it("a sighting noted before the chain's proof was asked counts for nothing: the grace runs from the first proven one, noted on the attempt", async () => {
    const { deps: d } = deps({ status: "EXPIRED" });
    // Noted half an hour ago by a clock an hour ahead of the credit's expiry, which the chain proves only now.
    const legacy = hold({ attempt: "a", creditExpiresAt: seconds() - 60 * 60, expiredSeenAt: Date.now() - 30 * 60 * 1000 });
    writeSplitHold(ACCOUNT, LINE, legacy);
    expect(await settleSplitHold(legacy, d)).toEqual({ kind: "expiring" });
    const first = Date.now();
    expect(await noteSplitHoldExpiry(createFakeLocks(), ACCOUNT, LINE, legacy, first)).toBe("noted");
    const noted = readSplitHoldRecord(ACCOUNT, LINE)!;
    expect(noted).toMatchObject({ expiryProvenAt: first, expiredSeenAt: legacy.expiredSeenAt });
    expect(await settleSplitHold(noted, { ...d, now: first + SPLIT_EXPIRY_CONFIRM_MS - 1 })).toEqual({ kind: "expiring" });
    expect(await settleSplitHold(noted, { ...d, now: first + SPLIT_EXPIRY_CONFIRM_MS })).toEqual({ kind: "released" });
  });

  it("a finalized time read that runs past its deadline is aborted and proves nothing: held, and nothing waits on it", async () => {
    const { chain, deps: d } = deps({ status: "EXPIRED" });
    let signal: AbortSignal | undefined;
    chain.finalizedTime = (_chainId, s) => {
      signal = s;
      return new Promise<number>(() => {});
    };
    await expect(readerFinalizedTime(chain, 20)(CHAIN.POLYGON)).rejects.toThrow(/took too long/);
    expect(signal?.aborted).toBe(true);
    const past = { creditExpiresAt: seconds() - 20 * 60, expiryProvenAt: Date.now() - 10 * SPLIT_EXPIRY_CONFIRM_MS };
    expect(await settleSplitHold(hold(past), { ...d, finalizedTime: readerFinalizedTime(chain, 20) })).toBeNull();
  });

  it("a reader whose finalized time throws at once rejects with that, with no deadline left to fire unhandled", async () => {
    const timers = vi.spyOn(globalThis, "setTimeout");
    try {
      const throwing = {
        finalizedTime: (): Promise<number> => {
          throw new Error("no chain here");
        },
      };
      await expect(readerFinalizedTime(throwing, 20)(CHAIN.POLYGON)).rejects.toThrow("no chain here");
      expect(timers).not.toHaveBeenCalled();
    } finally {
      timers.mockRestore();
    }
  });
});
