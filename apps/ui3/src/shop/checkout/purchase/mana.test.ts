// The MANA allowances the rails approve, on the fake chain and relay: a venue (here the CollectionStore) is approved in
// full as before; the CreditsManager a credits + MANA split pays through is approved exactly the gap, or nothing when its
// allowance already covers it; and the split encodes the credit's value and the gap apart.

import { decodeFunctionData, maxUint256, type Hex } from "viem";
import { describe, expect, it } from "vitest";

import { CREDITS_MANAGER_ABI, OFFCHAIN_META_TX_ABI } from "@shop/checkout/chain/abi";
import { CHAIN } from "@shop/model/chain";
import { BOOK } from "@shop/payment/fixtures";
import { requireContracts } from "../chain/contracts";
import { GaslessUnavailableError, provedNotSent } from "../chain/metatx";
import { createFakeChain, createFakeLocks, createFakeRelay, createFakeShop, createFakeWallet, creatorTrade, relayedApproval } from "../fakes";
import { buildGroupUseCreditsArgs, groupPurchases } from "./credits";
import type { SpendableCredit } from "./encoding";
import { tradeManaPrice } from "./cart";
import { buyMintWithCreditsAndMana, buyMintWithMana, buyWithCreditsAndMana, SaleCancelledError, splitTerms, type MintToBuy, type SplitDispatch } from "./mana";
import type { RailContext } from "./rail";

const MANA = 10n ** 18n;
const STORE = requireContracts(CHAIN.POLYGON).collectionStore!.address.toLowerCase();
const MANAGER = requireContracts(CHAIN.POLYGON).creditsManager!.address.toLowerCase();
const BUYER = BOOK.buyer.toLowerCase();

// 2.3 MANA, paid with a 1 MANA credit and a 1.3 MANA gap.
const mint: MintToBuy = { chainId: CHAIN.POLYGON, item: { collection: BOOK.collection, itemId: "7", priceWei: ((23n * MANA) / 10n).toString() } };
const credit: SpendableCredit = { id: `0x${"a1".repeat(32)}`, amount: MANA.toString(), availableAmount: MANA.toString(), expiresAt: 4_102_444_800, signature: `0x${"11".repeat(65)}` };
const GAP = (13n * MANA) / 10n;

function rail() {
  const chain = createFakeChain();
  const relay = createFakeRelay({ chain });
  const shop = createFakeShop({ reportCreditSubmission: () => ({ recorded: 1 }) });
  let clock = 0;
  const ctx: RailContext = {
    buyer: BUYER,
    provider: createFakeWallet(),
    reader: chain,
    client: shop.client,
    walletKind: "web3",
    relayerUrl: "https://node.test/transactions-api/v1",
    fetchImpl: relay.fetchImpl,
    // One browser's Web Locks: an exact grant runs under its own.
    locks: createFakeLocks(),
    gasless: true,
    wait: {
      timeoutMs: 30,
      pollMs: 10,
      now: () => clock,
      sleep: async (ms) => {
        clock += ms;
      },
    },
  };
  const key = (spender: string) => `${CHAIN.POLYGON}:${BUYER}:${spender}`;
  const approvals = () => relay.posts.map(relayedApproval).filter((a) => a !== null);
  return { chain, relay, ctx, key, approvals };
}

/** The useCredits arguments a relayed CreditsManager meta-transaction carries. */
function relayedUseCredits(body: unknown): { maxCreditedValue: bigint; maxUncreditedValue: bigint } {
  const data = (body as { transactionData: { params: [string, Hex] } }).transactionData.params[1];
  const meta = decodeFunctionData({ abi: OFFCHAIN_META_TX_ABI, data });
  const call = decodeFunctionData({ abi: CREDITS_MANAGER_ABI, data: meta.args[1] as Hex });
  return call.args[0] as { maxCreditedValue: bigint; maxUncreditedValue: bigint };
}

describe("a split's dispatch, and the bytes it leaves as", () => {
  /** A split of the store's mint, its allowance in place, with what its two hooks hear and when. */
  async function split(over: Partial<RailContext> = {}, hooks: { dispatch?: () => void; envelope?: () => void } = {}) {
    const r = rail();
    r.chain.allowances.set(r.key(MANAGER), 2n * MANA);
    // A wallet whose sends the chain confirms.
    let n = 0;
    const wallet = createFakeWallet({
      onSend: () => {
        const hash = `0x${(++n).toString(16).padStart(64, "0")}` as Hex;
        r.chain.receipts.set(hash, "success");
        return hash;
      },
    });
    const seen: { order: string[]; dispatched: SplitDispatch | null; envelope: Hex | null } = { order: [], dispatched: null, envelope: null };
    const fetchImpl = (async (...args: Parameters<typeof fetch>) => {
      seen.order.push("post");
      return r.relay.fetchImpl(...args);
    }) as typeof fetch;
    const done = buyMintWithCreditsAndMana(
      { ...r.ctx, provider: wallet, fetchImpl, ...over },
      {
        mint,
        credits: [credit],
        manaGapWei: GAP,
        onDispatch: (call) => {
          seen.order.push("dispatch");
          seen.dispatched = call;
          hooks.dispatch?.();
        },
        onEnvelope: (bytes) => {
          seen.order.push("envelope");
          seen.envelope = bytes;
          hooks.envelope?.();
        },
      },
    );
    const outcome = await done.then(
      () => null,
      (e: unknown) => e,
    );
    const asked = (method: string) => wallet.requests.filter((q) => q.method === method).length;
    return { ...r, wallet, seen, outcome, asked };
  }

  it("relayed: its call is kept, then the exact signed execution, before anything is posted; what leaves is that one call", async () => {
    const { relay, seen, outcome } = await split();
    expect(outcome).toBeNull();
    expect(seen.order).toEqual(["dispatch", "envelope", "post"]);
    const posted = (relay.posts[0] as { transactionData: { params: [string, Hex] } }).transactionData.params[1];
    expect(seen.envelope).toBe(posted);
    // The very call it kept (its external call's salt and all), never a second build of it.
    expect(decodeFunctionData({ abi: OFFCHAIN_META_TX_ABI, data: posted }).args[1]).toBe(seen.dispatched!.useCredits);
  });

  it("from the wallet: its call is kept, then the exact call data it is handed, before it is asked to send; that one call", async () => {
    const { seen, outcome, wallet, asked } = await split({ gasless: false });
    expect(outcome).toBeNull();
    expect(seen.order).toEqual(["dispatch", "envelope"]);
    expect(asked("eth_sendTransaction")).toBe(1);
    expect(seen.envelope).toBe(wallet.sent[0]!.data);
    expect(wallet.sent[0]!.data).toBe(seen.dispatched!.useCredits);
  });

  it("a purchase no longer its caller's once its allowance is in place (its terms changed, say) is neither kept, signed nor sent", async () => {
    const { relay, seen, outcome, asked } = await split({ live: () => false });
    expect(outcome).toBeInstanceOf(SaleCancelledError);
    expect(seen.order).toEqual([]);
    expect(asked("eth_signTypedData_v4") + asked("eth_sendTransaction")).toBe(0);
    expect(relay.posts).toHaveLength(0);
  });

  it("a purchase gone while its sale's signature is asked sends nothing: relayed, signed but never posted; from the wallet, never handed over", async () => {
    // Still the caller's once its allowance is in place; gone by the time its sale would leave.
    const liveOnce = () => {
      let calls = 0;
      return () => calls++ === 0;
    };
    const relayed = await split({ live: liveOnce() });
    expect(relayed.outcome).toBeInstanceOf(SaleCancelledError);
    expect(relayed.seen.order).toEqual(["dispatch"]);
    expect(relayed.asked("eth_signTypedData_v4")).toBe(1);
    expect(relayed.relay.posts).toHaveLength(0);
    const direct = await split({ gasless: false, live: liveOnce() });
    expect(direct.outcome).toBeInstanceOf(SaleCancelledError);
    expect(direct.asked("eth_sendTransaction")).toBe(0);
  });

  it("a throw while its call is kept stops it before anything is signed or sent", async () => {
    const stop = new Error("not recorded");
    const { relay, seen, outcome, asked } = await split({}, { dispatch: () => { throw stop; } });
    expect(outcome).toBe(stop);
    expect(seen.order).toEqual(["dispatch"]);
    expect(asked("eth_signTypedData_v4") + asked("eth_sendTransaction")).toBe(0);
    expect(relay.posts).toHaveLength(0);
  });

  it("a throw while its bytes are kept stops them unsent: relayed, signed but never posted; from the wallet, never handed over", async () => {
    const stop = new Error("not recorded");
    const relayed = await split({}, { envelope: () => { throw stop; } });
    expect(relayed.outcome).toBe(stop);
    expect(relayed.asked("eth_signTypedData_v4")).toBe(1);
    expect(relayed.relay.posts).toHaveLength(0);
    const direct = await split({ gasless: false }, { envelope: () => { throw stop; } });
    expect(direct.outcome).toBe(stop);
    expect(direct.asked("eth_sendTransaction")).toBe(0);
  });
});

describe("the terms a split settles on", () => {
  // The split vector: a listing of 101 MANA wei with a 10% coupon moves 91; quoted 30 cents, a 10-cent part.
  const listing = creatorTrade({ received: [{ assetType: 1, contractAddress: BOOK.mana, amount: "101", extra: "0x", beneficiary: BOOK.creator }] });
  const price = tradeManaPrice(listing, { discount: 100_000 } as Parameters<typeof tradeManaPrice>[1])!;
  const terms = (credit: bigint, confirmedWei = 61n) =>
    splitTerms({ priceWei: price, quotedCents: 30, partCents: 10, credit: { amount: credit.toString(), availableAmount: credit.toString() }, confirmedWei });

  it("101 wei less its 10% coupon is 91; the credit for 10 of 30 cents is 30 and the buyer's MANA 61", () => {
    expect(price).toBe(91n);
    expect(terms(30n)).toEqual({ creditWei: 30n, gapWei: 61n });
  });

  it("a credit that is not the node's share is refused, one wei either way", () => {
    expect(() => terms(29n)).toThrow(/not the share/);
    expect(() => terms(31n)).toThrow(/not the share/);
    expect(() => splitTerms({ priceWei: price, quotedCents: 30, partCents: 10, credit: { amount: "30", availableAmount: "29" }, confirmedWei: 61n })).toThrow(/not the share/);
  });

  it("a share that floors to zero is refused, even when the credit returned is zero too", () => {
    expect(() => splitTerms({ priceWei: 2n, quotedCents: 30, partCents: 10, credit: { amount: "0", availableAmount: "0" }, confirmedWei: 2n })).toThrow(/too small/);
  });

  it("a gap above what the buyer confirmed is refused, and a part outside the price never settles", () => {
    expect(() => terms(30n, 60n)).toThrow(/moved past/);
    for (const partCents of [0, 30, 31, -10]) {
      expect(() => splitTerms({ priceWei: price, quotedCents: 30, partCents, credit: { amount: "0", availableAmount: "0" }, confirmedWei: 61n })).toThrow(/inside the price/);
    }
  });
});

describe("what a failed relay proves", () => {
  it("only a relay that said nothing was sent (or was never asked) lets the wallet send the same thing instead", () => {
    for (const reason of ["relayer-rejected", "disabled", "contract-account", "signing-failed"] as const) {
      expect(provedNotSent(new GaslessUnavailableError("x", reason)), reason).toBe(true);
    }
    for (const reason of ["relayer-unreachable", "unknown"] as const) {
      expect(provedNotSent(new GaslessUnavailableError("x", reason)), reason).toBe(false);
    }
    expect(provedNotSent(new Error("the answer was lost"))).toBe(false);
  });
});

describe("the MANA a rail lets a contract pull", () => {
  it("a MANA-only mint still approves the CollectionStore in full, as before", async () => {
    const { ctx, approvals } = rail();
    await buyMintWithMana(ctx, { mint });
    expect(approvals()).toEqual([{ owner: BUYER, spender: STORE, amount: maxUint256 }]);
  });

  it("the split approves the CreditsManager exactly the gap, never an unlimited allowance, and encodes the credit and the gap apart", async () => {
    const { ctx, chain, relay, key, approvals } = rail();
    await buyMintWithCreditsAndMana(ctx, { mint, credits: [credit], manaGapWei: GAP });
    expect(approvals()).toEqual([{ owner: BUYER, spender: MANAGER, amount: GAP }]);
    expect(chain.allowances.get(key(MANAGER))).toBe(GAP);
    expect(relayedUseCredits(relay.posts.at(-1))).toMatchObject({ maxCreditedValue: MANA, maxUncreditedValue: GAP });
  });

  it("an allowance that already covers the gap approves nothing and is left as it is", async () => {
    const { ctx, chain, relay, key, approvals } = rail();
    chain.allowances.set(key(MANAGER), 2n * MANA);
    await buyMintWithCreditsAndMana(ctx, { mint, credits: [credit], manaGapWei: GAP });
    expect(approvals()).toEqual([]);
    expect(chain.allowances.get(key(MANAGER))).toBe(2n * MANA);
    expect(relay.posts).toHaveLength(1);
    expect(relayedUseCredits(relay.posts[0])).toMatchObject({ maxCreditedValue: MANA, maxUncreditedValue: GAP });
  });

  it("an allowance short of the gap (left by an earlier split) is set to exactly this gap", async () => {
    const { ctx, chain, key, approvals } = rail();
    chain.allowances.set(key(MANAGER), 1n);
    await buyMintWithCreditsAndMana(ctx, { mint, credits: [credit], manaGapWei: GAP });
    expect(approvals()).toEqual([{ owner: BUYER, spender: MANAGER, amount: GAP }]);
  });

  it("a trade paid with Credits and MANA approves exactly the gap and encodes the credit and the gap apart too", async () => {
    const { ctx, relay, approvals } = rail();
    await buyWithCreditsAndMana(ctx, { trade: creatorTrade(), credits: [credit], manaGapWei: GAP });
    expect(approvals()).toEqual([{ owner: BUYER, spender: MANAGER, amount: GAP }]);
    expect(relayedUseCredits(relay.posts.at(-1))).toMatchObject({ maxCreditedValue: MANA, maxUncreditedValue: GAP });
  });

  it("a confirmed gap never rides with other purchases: a group that names one settles it alone", () => {
    const split = { kind: "store" as const, item: mint.item, chainId: mint.chainId, credits: [credit], maxCreditedValue: MANA.toString(), maxUncreditedValue: GAP.toString() };
    const other = { kind: "store" as const, item: { ...mint.item, itemId: "8" }, chainId: mint.chainId, credits: [credit], maxCreditedValue: MANA.toString() };
    expect(() => buildGroupUseCreditsArgs(groupPurchases([split, other])[0]!, BUYER)).toThrow(/settles alone/);
    expect(buildGroupUseCreditsArgs(groupPurchases([split])[0]!, BUYER).args).toMatchObject({ maxCreditedValue: MANA, maxUncreditedValue: GAP });
  });

  it("no gap is refused before anything is approved or relayed", async () => {
    const { ctx, relay } = rail();
    await expect(buyMintWithCreditsAndMana(ctx, { mint, credits: [credit], manaGapWei: 0n })).rejects.toThrow(/No MANA gap/);
    expect(relay.posts).toHaveLength(0);
  });
});
