// from: shop@94584a5c:app/src/lib/coupons.ts
// A creator sale is a signed discount coupon over the creator's collections: the marketplace applies it to
// their PRIMARY listings at checkout (the server accepts coupons on COLLECTION_ITEM trades only), the
// listing itself is untouched, and the price reverts by itself when the window closes. The server stores
// and validates it (/shop-api/v1/coupons); this module builds, signs, posts, lists and ends one. v2 serves
// the coupon routes as a 503 unavailable envelope until its coupons indexer runs, which the pages show
// as an outage with its reason.

import { encodeAbiParameters, type Hex } from "viem";

import type { Eip1193Provider } from "../../../data/auth/wallet";
import { shopClient, type ShopClient } from "../../api/shop";
import type { ChainId } from "../../model/chain";
import { COLLECTION_DISCOUNT_COUPON, COUPON_MANAGER_ABI, COUPON_MANAGERS, type ContractData } from "../../market/lib/contracts";
import { readContract } from "../../market/lib/rpc";
import { networkOf, OFFCHAIN_MARKETPLACE_TYPES, randomSalt, type TradeChecks } from "../../market/lib/trades";
import { chainSalt, sendCall, signTypedData, waitForReceipt, walletAccount, type WaitOptions } from "../../market/lib/tx";
import { standardMerkleRoot } from "./merkle";

/** Percentage discount, in parts per million on chain: 30% is 300_000. The only type the Shop signs. */
export const DISCOUNT_TYPE_RATE = 1;
export const MIN_SALE_PCT = 5;
export const MAX_SALE_PCT = 70;
export const MAX_SALE_DURATION_MS = 30 * 24 * 60 * 60 * 1000;
/** The `uses` a sale is signed with when the creator sets no unit limit. */
export const UNCAPPED_USES = 1_000_000;

export type CreatorSaleStatus = "scheduled" | "active" | "ended" | "cancelled" | "exhausted" | "revoked";

/** A sale as the server reports it (`GET /v1/coupons`). `checks` timestamps are milliseconds. */
export type CreatorSale = {
  id: string;
  signer: string;
  chainId: number;
  network: string;
  checks: TradeChecks;
  couponManager: string;
  couponAddress: string;
  discountType: number;
  discount: number;
  root: string;
  collections: string[];
  signature: string;
  createdAt: number;
  state: { uses: number; cancelled: boolean; revoked: boolean; checkedAt: number } | null;
  status: CreatorSaleStatus;
};

/** What `POST /v1/coupons` takes. */
export type CouponCreation = {
  signer: string;
  chainId: ChainId;
  network: "ETHEREUM" | "MATIC";
  checks: TradeChecks;
  couponAddress: string;
  discountType: number;
  discount: number;
  collections: string[];
  signature: string;
};

export type CouponContracts = { couponManager: ContractData; collectionDiscountCoupon: string };

/** Upstream `getCouponContracts`: the manager of the marketplace version new listings go to (V3), or null. */
export function getCouponContracts(chainId: ChainId): CouponContracts | null {
  const manager = COUPON_MANAGERS.v3[chainId];
  const coupon = COLLECTION_DISCOUNT_COUPON[chainId];
  return manager && coupon ? { couponManager: manager, collectionDiscountCoupon: coupon.address } : null;
}

/** Upstream `findCouponContracts`: the deployment whose manager a stored sale names, or null when unknown. */
export function findCouponContracts(chainId: ChainId, address: string): CouponContracts | null {
  const coupon = COLLECTION_DISCOUNT_COUPON[chainId];
  if (!coupon) return null;
  const wanted = address.toLowerCase();
  for (const version of ["v3", "v2"] as const) {
    const manager = COUPON_MANAGERS[version][chainId];
    if (manager && manager.address === wanted) return { couponManager: manager, collectionDiscountCoupon: coupon.address };
  }
  return null;
}

/** Lower-cased and de-duplicated, so the same set always hashes to the same root. */
export function normalizeCollections(collections: string[]): string[] {
  return [...new Set(collections.map((c) => c.toLowerCase()))];
}

export type SaleInputProblem = "collections" | "pct" | "window" | "duration" | "uses";

export class SaleInputError extends Error {
  override name = "SaleInputError";
  constructor(readonly problem: SaleInputProblem) {
    super(`Invalid sale: ${problem}`);
  }
}

/** The Merkle root (OpenZeppelin StandardMerkleTree over `address` leaves) the coupon is signed over. */
export function collectionsRoot(collections: string[]): Hex {
  const unique = normalizeCollections(collections);
  if (unique.length === 0) throw new SaleInputError("collections");
  return standardMerkleRoot(
    unique.map((c) => [c]),
    ["address"],
  );
}

/** `abi.encode(CollectionDiscountCouponData)`: (discount type, discount ppm, collections root). */
export function encodeCouponData(discountPpm: number, root: string): Hex {
  return encodeAbiParameters([{ type: "uint256" }, { type: "uint256" }, { type: "bytes32" }], [BigInt(DISCOUNT_TYPE_RATE), BigInt(discountPpm), root as Hex]);
}

export function couponTypes() {
  return {
    Coupon: [
      { name: "checks", type: "Checks" },
      { name: "couponAddress", type: "address" },
      { name: "data", type: "bytes" },
    ],
    Checks: OFFCHAIN_MARKETPLACE_TYPES.Checks,
    ExternalCheck: OFFCHAIN_MARKETPLACE_TYPES.ExternalCheck,
  };
}

const toSeconds = (ms: number) => Math.floor(ms / 1000);
const pad32 = (hex: string): Hex => `0x${((hex.startsWith("0x") ? hex.slice(2) : hex) || "0").padStart(64, "0")}`;

export function couponDomain(chainId: ChainId, contracts: CouponContracts) {
  return { name: contracts.couponManager.name, version: contracts.couponManager.version, salt: chainSalt(chainId), verifyingContract: contracts.couponManager.address };
}

/** The EIP-712 values for a coupon: `checks` in seconds, as the contract reads them. */
export function couponTypedValues(checks: TradeChecks, couponAddress: string, data: string) {
  return {
    checks: {
      uses: BigInt(checks.uses),
      expiration: BigInt(toSeconds(checks.expiration)),
      effective: BigInt(toSeconds(checks.effective)),
      salt: pad32(checks.salt),
      contractSignatureIndex: BigInt(checks.contractSignatureIndex),
      signerSignatureIndex: BigInt(checks.signerSignatureIndex),
      allowedRoot: pad32(checks.allowedRoot || "0x"),
      externalChecks: [] as never[],
    },
    couponAddress,
    data,
  };
}

export type SaleTerms = {
  collections: string[];
  /** Whole percent, 5 to 70. */
  discountPct: number;
  /** Epoch ms; omitted starts now. */
  startsAtMs?: number;
  endsAtMs: number;
  /** Discounted purchases allowed; omitted means no limit. */
  uses?: number;
};

/** Upstream `validateSaleTerms`: what the contract, the server or the product would refuse. */
export function validateSaleTerms(terms: SaleTerms, now = Date.now()): void {
  if (normalizeCollections(terms.collections).length === 0) throw new SaleInputError("collections");
  if (!Number.isInteger(terms.discountPct) || terms.discountPct < MIN_SALE_PCT || terms.discountPct > MAX_SALE_PCT) throw new SaleInputError("pct");
  const startsAt = terms.startsAtMs ?? now;
  if (!(terms.endsAtMs > now) || !(terms.endsAtMs > startsAt)) throw new SaleInputError("window");
  if (terms.endsAtMs - Math.max(startsAt, now) > MAX_SALE_DURATION_MS) throw new SaleInputError("duration");
  if (terms.uses !== undefined && (!Number.isInteger(terms.uses) || terms.uses < 1)) throw new SaleInputError("uses");
}

/** Upstream `readCouponIndexes`: the manager's signature indexes, from the target chain. */
export async function readCouponIndexes(couponManager: ContractData, signer: string): Promise<{ contractSignatureIndex: number; signerSignatureIndex: number }> {
  const [contractIndex, signerIndex] = await Promise.all([
    readContract({ chainId: couponManager.chainId, address: couponManager.address, abi: COUPON_MANAGER_ABI, functionName: "contractSignatureIndex" }),
    readContract({ chainId: couponManager.chainId, address: couponManager.address, abi: COUPON_MANAGER_ABI, functionName: "signerSignatureIndex", args: [signer] }),
  ]);
  return { contractSignatureIndex: Number(contractIndex as bigint), signerSignatureIndex: Number(signerIndex as bigint) };
}

/** Upstream `createCollectionSale`: one EIP-712 signature, no transaction. Returns what `POST /v1/coupons` takes. */
export async function createCollectionSale(
  opts: { provider: Eip1193Provider; chainId: ChainId } & SaleTerms,
  deps: { readIndexes?: typeof readCouponIndexes; now?: () => number; salt?: () => Hex } = {},
): Promise<CouponCreation> {
  const now = deps.now ? deps.now() : Date.now();
  validateSaleTerms(opts, now);
  const contracts = getCouponContracts(opts.chainId);
  if (!contracts) throw new Error(`Coupons are not available on chain ${opts.chainId}`);
  const creator = await walletAccount(opts.provider);
  const collections = normalizeCollections(opts.collections);
  const indexes = await (deps.readIndexes ?? readCouponIndexes)(contracts.couponManager, creator);
  const checks: TradeChecks = {
    uses: opts.uses ?? UNCAPPED_USES,
    expiration: opts.endsAtMs,
    effective: opts.startsAtMs ?? now,
    salt: (deps.salt ?? randomSalt)(),
    contractSignatureIndex: indexes.contractSignatureIndex,
    signerSignatureIndex: indexes.signerSignatureIndex,
    allowedRoot: "0x",
    externalChecks: [],
  };
  const discount = opts.discountPct * 10_000;
  const data = encodeCouponData(discount, collectionsRoot(collections));
  const signature = await signTypedData(opts.provider, creator, {
    domain: couponDomain(opts.chainId, contracts),
    types: couponTypes(),
    primaryType: "Coupon",
    message: couponTypedValues(checks, contracts.collectionDiscountCoupon, data),
  });
  return {
    signer: creator,
    chainId: opts.chainId,
    network: networkOf(opts.chainId),
    checks,
    couponAddress: contracts.collectionDiscountCoupon,
    discountType: DISCOUNT_TYPE_RATE,
    discount,
    collections,
    signature,
  };
}

/** Upstream `postCoupon`: stores the signed sale (signed fetch, intent dcl:create-coupon). */
export async function postCoupon(payload: CouponCreation, c: ShopClient = shopClient()): Promise<CreatorSale> {
  const json = await c.market.createCoupon<{ ok?: boolean; data?: CreatorSale; message?: string }>(payload);
  if (!json?.ok || !json.data) throw new Error(json?.message ?? "The Shop did not store the sale");
  return json.data;
}

/** Upstream `fetchCreatorSales`: a creator's sales, newest first, with their last known state. */
export async function fetchCreatorSales(address: string, c: ShopClient = shopClient()): Promise<CreatorSale[]> {
  const json = await c.market.coupons<{ data?: CreatorSale[] }>({ signer: address.toLowerCase() });
  return json.data ?? [];
}

export function isSaleCapped(sale: Pick<CreatorSale, "checks">): boolean {
  return sale.checks.uses < UNCAPPED_USES;
}

/** Upstream `liveSaleStatus`: the server's status, refreshed against the clock. */
export function liveSaleStatus(sale: Pick<CreatorSale, "status" | "checks">, now = Date.now()): CreatorSaleStatus {
  if (sale.status === "cancelled" || sale.status === "revoked" || sale.status === "exhausted") return sale.status;
  if (sale.checks.expiration <= now) return "ended";
  if (sale.checks.effective > now) return "scheduled";
  return "active";
}

/**
 * Upstream `endSale`: `CouponManager.cancelSignature([coupon])` from the creator's wallet, on the manager the
 * sale was SIGNED against (cancelling on another one changes nothing where the coupon redeems). The wallet
 * must already be on the sale's chain.
 */
export async function endSale(opts: { provider: Eip1193Provider; sale: CreatorSale; wait?: WaitOptions }): Promise<string> {
  const { sale } = opts;
  const contracts = findCouponContracts(sale.chainId, sale.couponManager);
  if (!contracts) throw new Error(`The sale's coupon manager ${sale.couponManager} is not one this build knows on chain ${sale.chainId}`);
  const values = couponTypedValues(sale.checks, sale.couponAddress, encodeCouponData(sale.discount, sale.root));
  const onChainCoupon = {
    signature: sale.signature as Hex,
    checks: { ...values.checks, allowedProof: [] as Hex[], externalChecks: [] as never[] },
    couponAddress: sale.couponAddress as Hex,
    data: values.data as Hex,
    callerData: "0x" as Hex,
  };
  const hash = await sendCall(opts.provider, {
    chainId: sale.chainId,
    to: contracts.couponManager.address,
    abi: COUPON_MANAGER_ABI,
    functionName: "cancelSignature",
    args: [[onChainCoupon]],
  });
  return waitForReceipt(sale.chainId, hash, "Ending the sale", opts.wait);
}
