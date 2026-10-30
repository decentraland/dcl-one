// from: shop@94584a5c:app/src/lib/trade-encoding.ts, app/src/lib/trades.ts (valueForAsset)
// The on-chain encoding of every purchase the Shop settles: marketplace.accept / acceptWithCoupon, the
// CollectionStore.buy mint, and the CreditsManager.useCredits envelope around either. Shared by the
// buyer-submitted and the relayed rails so the bytes that move money come from one place. Upstream
// encoded with ethers' AbiCoder over hand-written tuple strings; here viem encodes against the ABIs in
// chain/abi.ts, whose selectors are checked against the contract fixtures.

import { encodeAbiParameters, encodeFunctionData, getAbiItem, pad, stringToHex, toFunctionSelector, toHex, type Hex } from "viem";

import type { ShopCoupon } from "@catalyst-api/market/ShopCoupon";
import { COLLECTION_STORE_ABI, CREDITS_MANAGER_ABI, MARKETPLACE_ABI } from "../chain/abi";

export { amoyGasOverrides } from "../chain/wallet";

/** The off-chain marketplace asset types (`src/shop/evm/trade.rs`, `ASSET_TYPE_*`). */
export const ASSET_TYPE = { ERC20: 1, USD_PEGGED_MANA: 2, ERC721: 3, COLLECTION_ITEM: 4 } as const;

export type TradeAssetWire = {
  assetType: number;
  contractAddress: string;
  extra?: string;
  amount?: string;
  tokenId?: string;
  itemId?: string;
  /** On-chain-shaped assets carry a generic value. */
  value?: string;
  beneficiary?: string;
};

export type TradeChecksWire = {
  uses: number;
  /** Stored in milliseconds, signed and checked on chain in seconds. */
  expiration: number;
  effective: number;
  salt: string;
  contractSignatureIndex: number;
  signerSignatureIndex: number;
  allowedRoot?: string;
  externalChecks?: { contractAddress: string; selector: string; value: string; required: boolean }[];
};

/** `GET /v1/trades/{id}`: a signed listing, exactly what the marketplace verifies. */
export type SignedTrade = {
  id: string;
  chainId: number;
  /** The marketplace version the trade was signed for. */
  contract: string;
  signer: string;
  signature: string;
  network?: string;
  type?: string;
  createdAt?: number;
  checks: TradeChecksWire;
  sent: TradeAssetWire[];
  received: TradeAssetWire[];
};

/** A creator's rate coupon with the Merkle proof for this listing's collection (the catalogue row). */
export type ListingCoupon = ShopCoupon;

/** The credit fields the CreditsManager spends: a v2 authorization's `credit`. */
export type SpendableCredit = {
  id: string;
  amount: string;
  availableAmount: string;
  /** Unix seconds. */
  expiresAt: number;
  signature: string;
  /** The CreditsManager the credit was signed for (its hash binds `address(this)`). */
  contract?: string;
};

export type CreditPurchase = { trade: SignedTrade; coupon?: ListingCoupon; credits: SpendableCredit[]; maxCreditedValue: string };

/** A CollectionStore mint; `priceWei` is read as late as possible, the store re-validates it on chain. */
export type StoreItemToBuy = { collection: string; itemId: string; priceWei: string };

export type StorePurchase = { item: StoreItemToBuy; credits: SpendableCredit[]; maxCreditedValue: string; chainId: number };

const ZERO32 = `0x${"0".repeat(64)}` as Hex;

/** Upstream `valueForAsset`: the one field of an asset the chain reads as `value`. */
export function valueForAsset(asset: TradeAssetWire): string {
  switch (asset.assetType) {
    case ASSET_TYPE.ERC721:
      return String(asset.tokenId ?? asset.value ?? "");
    case ASSET_TYPE.COLLECTION_ITEM:
      return String(asset.itemId ?? asset.value ?? "");
    case ASSET_TYPE.ERC20:
    case ASSET_TYPE.USD_PEGGED_MANA:
      return String(asset.amount ?? asset.value ?? "");
    default:
      throw new Error(`Unsupported assetType ${String(asset.assetType)}`);
  }
}

/** Milliseconds as stored, seconds as signed: seconds are ~1e9, milliseconds ~1e12. */
export function toChainSeconds(v: number | string): bigint {
  const n = Number(v);
  return BigInt(n > 1e12 ? Math.floor(n / 1000) : Math.floor(n));
}

function hex32(value: string | undefined): Hex {
  if (!value || value === "0x") return ZERO32;
  return pad(value as Hex, { size: 32 });
}

function onChainChecks(checks: TradeChecksWire) {
  return {
    uses: BigInt(checks.uses),
    expiration: toChainSeconds(checks.expiration),
    effective: toChainSeconds(checks.effective),
    salt: hex32(checks.salt),
    contractSignatureIndex: BigInt(checks.contractSignatureIndex),
    signerSignatureIndex: BigInt(checks.signerSignatureIndex),
    allowedRoot: hex32(checks.allowedRoot),
    allowedProof: [] as Hex[],
    externalChecks: (checks.externalChecks ?? []).map((c) => ({
      contractAddress: c.contractAddress as Hex,
      selector: c.selector as Hex,
      value: (c.value || "0x") as Hex,
      required: c.required,
    })),
  };
}

/** Upstream `getOnChainTrade`: the sent assets' beneficiary is the buyer; checks normalised. */
export function getOnChainTrade(trade: SignedTrade, buyer: string) {
  return {
    signer: trade.signer as Hex,
    signature: trade.signature as Hex,
    checks: onChainChecks(trade.checks),
    sent: trade.sent.map((a) => ({
      assetType: BigInt(a.assetType),
      contractAddress: a.contractAddress as Hex,
      value: BigInt(valueForAsset(a)),
      beneficiary: buyer as Hex,
      extra: (a.extra || "0x") as Hex,
    })),
    received: trade.received.map((a) => ({
      assetType: BigInt(a.assetType),
      contractAddress: a.contractAddress as Hex,
      value: BigInt(valueForAsset(a)),
      beneficiary: (a.beneficiary || buyer) as Hex,
      extra: (a.extra || "0x") as Hex,
    })),
  };
}

const ACCEPT = getAbiItem({ abi: MARKETPLACE_ABI, name: "accept" });
const ACCEPT_WITH_COUPON = getAbiItem({ abi: MARKETPLACE_ABI, name: "acceptWithCoupon" });
const STORE_BUY = getAbiItem({ abi: COLLECTION_STORE_ABI, name: "buy" });

export type ExternalCallBody = { selector: Hex; data: Hex };

/** `marketplace.accept(trades)` as selector + encoded arguments (the shape useCredits' external call takes). */
export function buildAcceptCalldata(trades: SignedTrade[], buyer: string): ExternalCallBody {
  return {
    selector: toFunctionSelector(ACCEPT),
    data: encodeAbiParameters(ACCEPT.inputs, [trades.map((t) => getOnChainTrade(t, buyer))]),
  };
}

/**
 * Upstream `getOnChainCoupon`. `data` is three static words; `callerData` wraps the proofs in a struct
 * (one extra offset word) and holds one proof per sent asset, since applyCoupon indexes them per asset.
 */
export function getOnChainCoupon(coupon: ListingCoupon, sentAssetCount = 1) {
  const proof = coupon.proof as Hex[];
  return {
    signature: coupon.signature as Hex,
    checks: onChainChecks(coupon.checks),
    couponAddress: coupon.couponAddress as Hex,
    data: encodeAbiParameters(
      [{ type: "uint256" }, { type: "uint256" }, { type: "bytes32" }],
      [BigInt(coupon.discountType), BigInt(coupon.discount), hex32(coupon.root)],
    ),
    callerData: encodeAbiParameters(
      [{ type: "tuple", components: [{ name: "proofs", type: "bytes32[][]" }] }],
      [{ proofs: Array.from({ length: Math.max(1, sentAssetCount) }, () => proof) }],
    ),
  };
}

/** `marketplace.acceptWithCoupon(trades, coupons)`: one coupon per trade, index for index. */
export function buildAcceptWithCouponCalldata(trades: SignedTrade[], coupons: ListingCoupon[], buyer: string): ExternalCallBody {
  if (trades.length !== coupons.length) {
    throw new Error(`acceptWithCoupon needs one coupon per trade, got ${coupons.length} for ${trades.length} trades`);
  }
  return {
    selector: toFunctionSelector(ACCEPT_WITH_COUPON),
    data: encodeAbiParameters(ACCEPT_WITH_COUPON.inputs, [
      trades.map((t) => getOnChainTrade(t, buyer)),
      coupons.map((c, i) => getOnChainCoupon(c, trades[i]!.sent.length)),
    ]),
  };
}

/** Full calldata (selector included) for a direct or relayed marketplace call. */
export function encodeMarketplaceCall(trades: SignedTrade[], buyer: string, coupons?: ListingCoupon[]): Hex {
  const body = coupons?.length ? buildAcceptWithCouponCalldata(trades, coupons, buyer) : buildAcceptCalldata(trades, buyer);
  return `${body.selector}${body.data.slice(2)}` as Hex;
}

/** Upstream `idToSalt`: a credit id as the bytes32 the manager hashes. */
export function idToSalt(id: string): Hex {
  if (!id) return ZERO32;
  return id.startsWith("0x") ? pad(id as Hex, { size: 32 }) : pad(stringToHex(id), { size: 32 });
}

/** Upstream `itemsToBuyArg`: the buyer is every beneficiary, whoever sends the transaction. */
export function itemsToBuyArg(items: StoreItemToBuy[], buyer: string) {
  return items.map((i) => ({
    collection: i.collection as Hex,
    ids: [BigInt(i.itemId)],
    prices: [BigInt(i.priceWei)],
    beneficiaries: [buyer as Hex],
  }));
}

export function buildStoreBuyCalldata(items: StoreItemToBuy[], buyer: string): ExternalCallBody {
  return { selector: toFunctionSelector(STORE_BUY), data: encodeAbiParameters(STORE_BUY.inputs, [itemsToBuyArg(items, buyer)]) };
}

/** `CollectionStore.buy(items)` as full calldata, for the MANA rails that call the store directly. */
export function encodeStoreBuy(items: StoreItemToBuy[], buyer: string): Hex {
  return encodeFunctionData({ abi: COLLECTION_STORE_ABI, functionName: "buy", args: [itemsToBuyArg(items, buyer)] });
}

export type UseCreditsArgs = {
  credits: { value: bigint; expiresAt: bigint; salt: Hex }[];
  creditsSignatures: Hex[];
  externalCall: { target: Hex; selector: Hex; data: Hex; expiresAt: bigint; salt: Hex };
  customExternalCallSignature: Hex;
  maxUncreditedValue: bigint;
  maxCreditedValue: bigint;
};

function randomSalt(): Hex {
  const bytes = new Uint8Array(32);
  globalThis.crypto.getRandomValues(bytes);
  return toHex(bytes);
}

export type EnvelopeOptions = { now?: () => number; salt?: () => Hex };

/**
 * The shared useCredits envelope. `maxUncreditedValue` (MANA the buyer pays from the wallet) is derived
 * from the server-sized `maxCreditedValue` minus what the credits hold, never from an item price: a
 * price-derived gap silently charges the difference in MANA.
 */
export function wrapInUseCredits(
  opts: { target: string; call: ExternalCallBody; credits: SpendableCredit[]; maxCreditedValue: string },
  env: EnvelopeOptions = {},
): UseCreditsArgs {
  const now = env.now ?? Date.now;
  const sumAvailable = opts.credits.reduce((acc, c) => acc + BigInt(c.availableAmount), 0n);
  const maxCredited = BigInt(opts.maxCreditedValue);
  const uncredited = maxCredited - sumAvailable;
  return {
    credits: opts.credits.map((c) => ({ value: BigInt(c.amount), expiresAt: BigInt(Math.floor(Number(c.expiresAt))), salt: idToSalt(c.id) })),
    creditsSignatures: opts.credits.map((c) => c.signature as Hex),
    externalCall: {
      target: opts.target as Hex,
      selector: opts.call.selector,
      data: opts.call.data,
      expiresAt: BigInt(Math.floor(now() / 1000) + 60 * 60 * 24),
      salt: (env.salt ?? randomSalt)(),
    },
    customExternalCallSignature: "0x",
    maxUncreditedValue: uncredited < 0n ? 0n : uncredited,
    maxCreditedValue: maxCredited,
  };
}

/** useCredits arguments for trades on ONE marketplace (accept, or acceptWithCoupon when discounted). */
export function buildUseCreditsArgs(
  marketplaceAddress: string,
  trades: SignedTrade[],
  buyer: string,
  credits: SpendableCredit[],
  maxCreditedValue: string,
  coupons?: ListingCoupon[],
  env?: EnvelopeOptions,
): UseCreditsArgs {
  const call = coupons?.length ? buildAcceptWithCouponCalldata(trades, coupons, buyer) : buildAcceptCalldata(trades, buyer);
  return wrapInUseCredits({ target: marketplaceAddress, call, credits, maxCreditedValue }, env);
}

/** useCredits arguments for a batch of CollectionStore mints. */
export function buildStoreUseCreditsArgs(
  collectionStoreAddress: string,
  items: StoreItemToBuy[],
  buyer: string,
  credits: SpendableCredit[],
  maxCreditedValue: string,
  env?: EnvelopeOptions,
): UseCreditsArgs {
  return wrapInUseCredits({ target: collectionStoreAddress, call: buildStoreBuyCalldata(items, buyer), credits, maxCreditedValue }, env);
}

export function encodeUseCredits(args: UseCreditsArgs): Hex {
  return encodeFunctionData({ abi: CREDITS_MANAGER_ABI, functionName: "useCredits", args: [args] });
}
