// The split sale hold every surface shares: one claim per account and line at a time, the record read inside the claim,
// writes and clears scoped to their attempt, and what settles a hold.

import { afterEach, describe, expect, it, vi } from "vitest";

import { CHAIN } from "@shop/model/chain";
import { itemAsset } from "@shop/model/asset";
import { BOOK } from "@shop/payment/fixtures";
import { createFakeChain, createFakeLocks, createFakeShop } from "../fakes";
import {
  claimSplitHold,
  clearSettledSplitHold,
  clearSplitHold,
  readSplitHold,
  readSplitHoldRecord,
  settleSplitHold,
  SPLIT_HOLD_MS,
  splitHoldKey,
  splitHoldLine,
  splitHoldLockName,
  writeSplitHold,
  type SplitHold,
} from "./splitHold";

afterEach(() => {
  localStorage.clear();
  vi.restoreAllMocks();
});

const ACCOUNT = BOOK.buyer;
const ASSET = itemAsset(CHAIN.POLYGON, BOOK.collection, "7");
const LINE = splitHoldLine(ASSET, { acquisition: "store" });
const CREDIT = `0x${"c1".repeat(32)}`;
const seconds = () => Math.floor(Date.now() / 1000);
const hold = (over: Partial<SplitHold> = {}): SplitHold => ({ at: Date.now(), attempt: "a", txHash: null, chainId: CHAIN.POLYGON, creditId: CREDIT, creditExpiresAt: seconds() + 300, ...over });

describe("what a hold is kept under", () => {
  it("the item, whatever listing a surface buys it through: the same item is one hold, another item another", () => {
    const byTrade = splitHoldLine(ASSET, { tradeId: "t-1", acquisition: "trade" });
    // The web's item page may sell it through a creator's trade while the in-world Shop picks the store's mint.
    expect(splitHoldLine(ASSET, { acquisition: "store" })).toBe(byTrade);
    expect(splitHoldLine(ASSET, { tradeId: "t-2" })).toBe(byTrade);
    expect(splitHoldLine(ASSET)).toBe(byTrade);
    expect(byTrade).toBe(`${CHAIN.POLYGON}:${BOOK.collection}:item:7`);
    expect(splitHoldLine(itemAsset(CHAIN.POLYGON, BOOK.collection, "8"))).not.toBe(byTrade);
    // A hold written through one listing blocks a claim through another.
    writeSplitHold(ACCOUNT, splitHoldLine(ASSET, { tradeId: "t-1" }), hold());
    expect(readSplitHold(ACCOUNT, splitHoldLine(ASSET, { acquisition: "store" }))).not.toBeNull();
    // The key and the lock name lowercase the account, so a checksummed address finds the same hold.
    expect(splitHoldKey(ACCOUNT.toUpperCase().replace("0X", "0x"), LINE)).toBe(splitHoldKey(ACCOUNT, LINE));
    expect(splitHoldLockName(ACCOUNT.toUpperCase().replace("0X", "0x"), LINE)).toBe(splitHoldLockName(ACCOUNT, LINE));
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

  it("an outcome waits while another run holds the line", async () => {
    const locks = createFakeLocks();
    writeSplitHold(ACCOUNT, LINE, hold());
    locks.held.add(splitHoldLockName(ACCOUNT, LINE));
    expect(await clearSettledSplitHold(locks, ACCOUNT, LINE, hold())).toEqual({ kind: "busy" });
    expect(readSplitHoldRecord(ACCOUNT, LINE)).not.toBeNull();
  });
});

describe("what settles a hold", () => {
  function deps(listed: { status: "PENDING" | "SETTLED" | "EXPIRED" } | null) {
    const chain = createFakeChain();
    const shop = createFakeShop({
      userPurchases: () => ({
        purchases: listed ? [{ id: `${CREDIT}:0`, status: listed.status, usdCents: 50, createdAt: 1, txHash: listed.status === "SETTLED" ? `0x${"ab".repeat(32)}` : null }] : [],
        total: listed ? 1 : 0,
      }),
    });
    return { chain, shop, deps: { account: ACCOUNT, reader: chain, client: shop.client } };
  }
  const HASH = `0x${"5a".repeat(32)}`;

  it("its own receipt: mined completes it, reverted says its credit went unspent", async () => {
    const ok = deps(null);
    ok.chain.receipts.set(HASH, "success");
    expect(await settleSplitHold(hold({ txHash: HASH }), ok.deps)).toEqual({ kind: "spent", txHash: HASH });
    const reverted = deps(null);
    reverted.chain.receipts.set(HASH, "reverted");
    expect(await settleSplitHold(hold({ txHash: HASH }), reverted.deps)).toEqual({ kind: "reverted" });
  });

  it("no receipt (or a read that fails) while its credit can still be spent: nothing says, the node is not asked", async () => {
    const { chain, shop, deps: d } = deps({ status: "EXPIRED" });
    chain.receipt = async () => {
      throw new Error("the RPC is down");
    };
    expect(await settleSplitHold(hold({ txHash: HASH }), d)).toBeNull();
    expect(shop.log.some((r) => r.name === "userPurchases")).toBe(false);
  });

  it("once its credit can no longer be spent, the node's record of it decides: spent, released, or still held", async () => {
    const over = hold({ txHash: HASH, creditExpiresAt: seconds() - 1 });
    expect(await settleSplitHold(over, deps({ status: "SETTLED" }).deps)).toEqual({ kind: "spent", txHash: `0x${"ab".repeat(32)}` });
    expect(await settleSplitHold(over, deps({ status: "EXPIRED" }).deps)).toEqual({ kind: "released" });
    expect(await settleSplitHold(over, deps({ status: "PENDING" }).deps)).toBeNull();
    expect(await settleSplitHold(over, deps(null).deps)).toBeNull();
  });
});
