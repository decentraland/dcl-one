// What a purchase transaction settled, read from its own calldata and receipt rather than from a balance
// difference: the marketplace call it made (accept or acceptWithCoupon), the coupon it carried, and the MANA the
// contract charges for it. Pure functions; the journey fetches the transaction, receipt and feed round.

import { concat, decodeAbiParameters, decodeFunctionData, encodeAbiParameters, keccak256, parseAbi, toFunctionSelector, type Hex } from "viem";

export const MARKETPLACE_ABI = parseAbi([
  "struct ExternalCheck { address contractAddress; bytes4 selector; bytes value; bool required; }",
  "struct Checks { uint256 uses; uint256 expiration; uint256 effective; bytes32 salt; uint256 contractSignatureIndex; uint256 signerSignatureIndex; bytes32 allowedRoot; bytes32[] allowedProof; ExternalCheck[] externalChecks; }",
  "struct Asset { uint256 assetType; address contractAddress; uint256 value; address beneficiary; bytes extra; }",
  "struct Trade { address signer; bytes signature; Checks checks; Asset[] sent; Asset[] received; }",
  "struct Coupon { bytes signature; Checks checks; address couponAddress; bytes data; bytes callerData; }",
  "function accept(Trade[] _trades)",
  "function acceptWithCoupon(Trade[] _trades, Coupon[] _coupons)",
  "function executeMetaTransaction(address userAddress, bytes functionData, bytes signature)",
]);

export const ASSET_TYPE = { ERC20: 1n, USD_PEGGED_MANA: 2n, COLLECTION_ITEM: 4n } as const;
const PPM = 1_000_000n;

export type Settled = {
  call: "accept" | "acceptWithCoupon";
  trades: readonly { signer: Hex; sent: readonly { assetType: bigint; contractAddress: Hex; value: bigint }[]; received: readonly { assetType: bigint; contractAddress: Hex; value: bigint }[] }[];
  coupons: readonly { couponAddress: Hex; data: Hex; callerData: Hex }[];
};

/** The marketplace call a transaction made, unwrapping a relayed `executeMetaTransaction`. */
export function settledCall(input: Hex): Settled {
  let data = input;
  if (data.slice(0, 10) === toFunctionSelector("function executeMetaTransaction(address,bytes,bytes)")) {
    const meta = decodeFunctionData({ abi: MARKETPLACE_ABI, data });
    data = meta.args[1] as Hex;
  }
  const call = decodeFunctionData({ abi: MARKETPLACE_ABI, data });
  if (call.functionName === "accept") return { call: "accept", trades: call.args[0], coupons: [] };
  if (call.functionName === "acceptWithCoupon") return { call: "acceptWithCoupon", trades: call.args[0], coupons: call.args[1] };
  throw new Error(`the transaction called ${call.functionName}, not a marketplace purchase`);
}

/**
 * Where a settled call sells `itemId` of `collection`: the trade and the position of the asset in its `sent`,
 * which is also the position of its proof in the coupon's `callerData`. Exactly one such asset, or an error.
 */
export function soldItem(settled: Settled, collection: Hex, itemId: bigint): { trade: number; asset: number } {
  const found: { trade: number; asset: number }[] = [];
  settled.trades.forEach((t, trade) =>
    t.sent.forEach((a, asset) => {
      if (a.assetType === ASSET_TYPE.COLLECTION_ITEM && a.contractAddress.toLowerCase() === collection.toLowerCase() && a.value === itemId) found.push({ trade, asset });
    }),
  );
  if (found.length !== 1) throw new Error(`the call sells item ${itemId} of ${collection} ${found.length} times, not once`);
  return found[0]!;
}

/** `CollectionDiscountCouponData`: `(discountType, discount, root)`. */
export function couponData(data: Hex): { discountType: bigint; discount: bigint; root: Hex } {
  const [discountType, discount, root] = decodeAbiParameters([{ type: "uint256" }, { type: "uint256" }, { type: "bytes32" }], data);
  return { discountType, discount, root };
}

/** `CollectionDiscountCouponCallerData`: one collection proof per sent asset. */
export function couponProofs(callerData: Hex): readonly (readonly Hex[])[] {
  const [{ proofs }] = decodeAbiParameters([{ type: "tuple", components: [{ name: "proofs", type: "bytes32[][]" }] }], callerData);
  return proofs;
}

/** OpenZeppelin StandardMerkleTree over `address` leaves: `keccak256(keccak256(abi.encode(address)))`. */
export function collectionLeaf(collection: Hex): Hex {
  return keccak256(keccak256(encodeAbiParameters([{ type: "address" }], [collection])));
}

/** Whether `proof` places `collection` under `root`, hashing each pair in sorted order. */
export function collectionInRoot(collection: Hex, proof: readonly Hex[], root: Hex): boolean {
  let node = collectionLeaf(collection);
  for (const sibling of proof) node = BigInt(node) < BigInt(sibling) ? keccak256(concat([node, sibling])) : keccak256(concat([sibling, node]));
  return node.toLowerCase() === root.toLowerCase();
}

/** A rate coupon as the contract applies it to each received amount: `amount - floor(amount * ppm / 1e6)`. */
export function discounted(amount: bigint, ppm: bigint): bigint {
  return amount - (amount * ppm) / PPM;
}

/** A USD-pegged amount in MANA at a feed round: `usd * 10^decimals / answer`, flooring. */
export function usdToMana(usdWei: bigint, answer: bigint, decimals: number): bigint {
  return (usdWei * 10n ** BigInt(decimals)) / answer;
}

/**
 * The MANA the buyer pays for `received`: the coupon first, on each amount as signed, and only then the
 * conversion of a USD-pegged amount at the round the execution read.
 */
export function chargedMana(received: Settled["trades"][number]["received"], ppm: bigint | null, round: { answer: bigint; decimals: number } | null): bigint {
  let total = 0n;
  for (const asset of received) {
    const value = ppm === null ? asset.value : discounted(asset.value, ppm);
    if (asset.assetType === ASSET_TYPE.USD_PEGGED_MANA) {
      if (!round) throw new Error("a USD-pegged amount needs the feed round of the execution");
      total += usdToMana(value, round.answer, round.decimals);
    } else if (asset.assetType === ASSET_TYPE.ERC20) {
      total += value;
    }
  }
  return total;
}
