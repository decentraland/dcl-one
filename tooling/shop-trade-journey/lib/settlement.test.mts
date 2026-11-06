import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { concat, encodeAbiParameters, encodeFunctionData, keccak256, toFunctionSelector, type Hex } from "viem";
import { ASSET_TYPE, chargedMana, collectionInRoot, collectionLeaf, couponData, couponProofs, discounted, MARKETPLACE_ABI, settledCall, soldItem } from "./settlement.mts";

const A = "0x00000000000000000000000000000000000000aa" as Hex;
const B = "0x00000000000000000000000000000000000000bb" as Hex;
const COUPON = "0xc914507fe297b2dddd1232ac3a8903f1c125e794" as Hex;
const checks = { uses: 1n, expiration: 1n, effective: 0n, salt: `0x${"00".repeat(32)}` as Hex, contractSignatureIndex: 0n, signerSignatureIndex: 0n, allowedRoot: `0x${"00".repeat(32)}` as Hex, allowedProof: [], externalChecks: [] };
const trade = {
  signer: A,
  signature: "0x" as Hex,
  checks,
  sent: [{ assetType: 4n, contractAddress: B, value: 0n, beneficiary: A, extra: "0x" as Hex }],
  received: [{ assetType: ASSET_TYPE.USD_PEGGED_MANA, contractAddress: B, value: 7n, beneficiary: A, extra: "0x" as Hex }],
};

describe("the settled marketplace call", () => {
  it("names the selectors seen on Amoy", () => {
    assert.equal(toFunctionSelector(MARKETPLACE_ABI.find((f) => f.type === "function" && f.name === "accept")!), "0x961a547e");
    assert.equal(toFunctionSelector(MARKETPLACE_ABI.find((f) => f.type === "function" && f.name === "acceptWithCoupon")!), "0xad1e8700");
  });

  it("unwraps a relayed acceptWithCoupon and reads its coupon", () => {
    const root = keccak256(concat([collectionLeaf(A), collectionLeaf(B)].sort((x, y) => (BigInt(x) < BigInt(y) ? -1 : 1)) as [Hex, Hex]));
    const coupon = {
      signature: "0x" as Hex,
      checks,
      couponAddress: COUPON,
      data: encodeAbiParameters([{ type: "uint256" }, { type: "uint256" }, { type: "bytes32" }], [1n, 100_000n, root]),
      callerData: encodeAbiParameters([{ type: "tuple", components: [{ name: "proofs", type: "bytes32[][]" }] }], [{ proofs: [[collectionLeaf(A)]] }]),
    };
    const inner = encodeFunctionData({ abi: MARKETPLACE_ABI, functionName: "acceptWithCoupon", args: [[trade], [coupon]] });
    const relayed = encodeFunctionData({ abi: MARKETPLACE_ABI, functionName: "executeMetaTransaction", args: [A, inner, "0x"] });
    const settled = settledCall(relayed);
    assert.equal(settled.call, "acceptWithCoupon");
    assert.equal(settled.coupons[0]!.couponAddress.toLowerCase(), COUPON);
    assert.deepEqual(couponData(settled.coupons[0]!.data), { discountType: 1n, discount: 100_000n, root });
    assert.ok(collectionInRoot(B, couponProofs(settled.coupons[0]!.callerData)[0]!, root), "B is in the tree with A's leaf as its proof");
    assert.ok(!collectionInRoot("0x00000000000000000000000000000000000000cc", [collectionLeaf(A)], root));
    assert.equal(settledCall(encodeFunctionData({ abi: MARKETPLACE_ABI, functionName: "accept", args: [[trade]] })).call, "accept");
  });
});

describe("the item a settled call sells", () => {
  const item = (contractAddress: Hex, value: bigint) => ({ assetType: ASSET_TYPE.COLLECTION_ITEM, contractAddress, value, beneficiary: A, extra: "0x" as Hex });
  const settled = (sent: ReturnType<typeof item>[][]) => ({ call: "acceptWithCoupon" as const, coupons: [], trades: sent.map((s) => ({ ...trade, sent: s })) });

  it("names the trade and the position of the asset, which is the position of its proof", () => {
    assert.deepEqual(soldItem(settled([[item(A, 0n)], [item(A, 2n), item(B, 1n)]]), B, 1n), { trade: 1, asset: 1 });
  });

  it("refuses another item, another collection, and an item sold twice", () => {
    assert.throws(() => soldItem(settled([[item(B, 0n)]]), B, 1n), /0 times/);
    assert.throws(() => soldItem(settled([[item(A, 1n)]]), B, 1n), /0 times/);
    assert.throws(() => soldItem(settled([[item(B, 1n)], [item(B, 1n)]]), B, 1n), /2 times/);
  });
});

describe("the MANA a purchase is charged", () => {
  it("applies the coupon to the signed amount before a USD price converts", () => {
    // 7 USD-wei at 0.3 USD/MANA (8 decimals), 50% off: 7 - floor(3.5) = 4, then floor(4e8 / 3e7) = 13.
    // Converting first (floor(7e8 / 3e7) = 23) and then discounting (23 - 11) would give 12.
    const round = { answer: 30_000_000n, decimals: 8 };
    assert.equal(chargedMana(trade.received, 500_000n, round), 13n);
    assert.equal(discounted(chargedMana(trade.received, null, round), 500_000n), 12n);
  });

  it("charges a MANA price less the coupon, and needs a round only for a USD price", () => {
    const mana = [{ assetType: ASSET_TYPE.ERC20, contractAddress: B, value: 10n ** 18n }];
    assert.equal(chargedMana(mana, 100_000n, null), 9n * 10n ** 17n);
    assert.throws(() => chargedMana(trade.received, null, null), /feed round/);
  });
});
