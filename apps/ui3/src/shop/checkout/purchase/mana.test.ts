// The MANA allowances the rails approve, on the fake chain and relay: a venue (here the CollectionStore) is approved in
// full as before; the CreditsManager a credits + MANA split pays through is approved exactly the gap, or nothing when its
// allowance already covers it; and the split encodes the credit's value and the gap apart.

import { decodeFunctionData, maxUint256, type Hex } from "viem";
import { describe, expect, it } from "vitest";

import { CREDITS_MANAGER_ABI, OFFCHAIN_META_TX_ABI } from "@shop/checkout/chain/abi";
import { CHAIN } from "@shop/model/chain";
import { BOOK } from "@shop/payment/fixtures";
import { requireContracts } from "../chain/contracts";
import { createFakeChain, createFakeRelay, createFakeShop, createFakeWallet, creatorTrade, relayedApproval } from "../fakes";
import { buildGroupUseCreditsArgs, groupPurchases } from "./credits";
import type { SpendableCredit } from "./encoding";
import { tradeManaPrice } from "./cart";
import { buyMintWithCreditsAndMana, buyMintWithMana, buyWithCreditsAndMana, splitTerms, type MintToBuy } from "./mana";
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

  it("a gap above what the buyer confirmed is refused, and a part outside the price never settles", () => {
    expect(() => terms(30n, 60n)).toThrow(/moved past/);
    for (const partCents of [0, 30, 31, -10]) {
      expect(() => splitTerms({ priceWei: price, quotedCents: 30, partCents, credit: { amount: "0", availableAmount: "0" }, confirmedWei: 61n })).toThrow(/inside the price/);
    }
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
