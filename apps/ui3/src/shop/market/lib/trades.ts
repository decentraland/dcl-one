// from: shop@94584a5c:app/src/lib/trades.ts, app/src/lib/buy.ts (cancelListing), app/src/lib/api.ts (postTrade, fetchTrade),
//       app/src/lib/trade-encoding.ts (getOnChainTrade); marketplace@f3956826:webapp/src/utils/trades.ts,
//       webapp/src/modules/bid/utils.ts (createBidTrade), webapp/src/modules/order/sagas.ts (createOrder)
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.

import { encodeFunctionData, type Hex } from "viem";

import type { Eip1193Provider } from "../../../data/auth/wallet";
import { isShopApiError } from "../../api/client";
import { shopClient } from "../../api/shop";
import { isPolygon, type ChainId } from "../../model/chain";
import { requireChain } from "../../wallet/chain";
import {
  COLLECTION_V2_ABI,
  collectionV2,
  ERC721_ABI,
  latestOffChainMarketplace,
  manaToken,
  MARKETPLACE_ABI,
  marketplaceForTrade,
  type ContractData,
} from "./contracts";
import { MetaTransactionError, sendMetaTransaction } from "./metaTx";
import { readContract } from "./rpc";
import { chainSalt, declined, sendCall, signTypedData, waitForReceipt, walletAccount, type WaitOptions } from "./tx";

export const TRADE_ASSET = { ERC20: 1, USD_PEGGED_MANA: 2, ERC721: 3, COLLECTION_ITEM: 4 } as const;
export type TradeAssetType = (typeof TRADE_ASSET)[keyof typeof TRADE_ASSET];

export type TradeType = "bid" | "public_nft_order" | "public_item_order";
export type TradeNetwork = "ETHEREUM" | "MATIC";

export type TradeAsset =
  | { assetType: typeof TRADE_ASSET.ERC20 | typeof TRADE_ASSET.USD_PEGGED_MANA; contractAddress: string; amount: string; extra: string }
  | { assetType: typeof TRADE_ASSET.ERC721; contractAddress: string; tokenId: string; extra: string }
  | { assetType: typeof TRADE_ASSET.COLLECTION_ITEM; contractAddress: string; itemId: string; extra: string };

export type TradeReceivedAsset = TradeAsset & { beneficiary: string };

export type ExternalCheck = { contractAddress: string; selector: string; value: string; required: boolean };

export type TradeChecks = {
  uses: number;
  expiration: number;
  effective: number;
  salt: string;
  contractSignatureIndex: number;
  signerSignatureIndex: number;
  allowedRoot: string;
  allowedProof?: string[] | null;
  externalChecks: ExternalCheck[];
};

export type TradeCreation = {
  signer: string;
  signature: string;
  type: TradeType;
  network: TradeNetwork;
  chainId: ChainId;
  checks: TradeChecks;
  sent: TradeAsset[];
  received: TradeReceivedAsset[];
};

export type Trade = TradeCreation & { id: string; contract: string; createdAt?: number };

const ZERO_ADDRESS = "0x0000000000000000000000000000000000000000";
const toSeconds = (ms: number) => Math.floor(ms / 1000);

function pad32(hex: string): Hex {
  const body = (hex.startsWith("0x") ? hex.slice(2) : hex) || "0";
  if (body.length > 64) throw new Error(`not a 32-byte value: ${hex}`);
  return `0x${body.padStart(64, "0")}`;
}

function bytes(hex: string): Hex {
  return (hex && hex !== "" ? hex : "0x") as Hex;
}

export function valueForAsset(asset: TradeAsset): string {
  switch (asset.assetType) {
    case TRADE_ASSET.ERC721:
      return asset.tokenId;
    case TRADE_ASSET.COLLECTION_ITEM:
      return asset.itemId;
    case TRADE_ASSET.ERC20:
    case TRADE_ASSET.USD_PEGGED_MANA:
      return asset.amount;
    default:
      throw new Error(`Unsupported assetType ${String((asset as { assetType: unknown }).assetType)}`);
  }
}

export const OFFCHAIN_MARKETPLACE_TYPES = {
  Trade: [
    { name: "checks", type: "Checks" },
    { name: "sent", type: "AssetWithoutBeneficiary[]" },
    { name: "received", type: "Asset[]" },
  ],
  Asset: [
    { name: "assetType", type: "uint256" },
    { name: "contractAddress", type: "address" },
    { name: "value", type: "uint256" },
    { name: "extra", type: "bytes" },
    { name: "beneficiary", type: "address" },
  ],
  AssetWithoutBeneficiary: [
    { name: "assetType", type: "uint256" },
    { name: "contractAddress", type: "address" },
    { name: "value", type: "uint256" },
    { name: "extra", type: "bytes" },
  ],
  Checks: [
    { name: "uses", type: "uint256" },
    { name: "expiration", type: "uint256" },
    { name: "effective", type: "uint256" },
    { name: "salt", type: "bytes32" },
    { name: "contractSignatureIndex", type: "uint256" },
    { name: "signerSignatureIndex", type: "uint256" },
    { name: "allowedRoot", type: "bytes32" },
    { name: "externalChecks", type: "ExternalCheck[]" },
  ],
  ExternalCheck: [
    { name: "contractAddress", type: "address" },
    { name: "selector", type: "bytes4" },
    { name: "value", type: "bytes" },
    { name: "required", type: "bool" },
  ],
} as const;

export function generateTradeValues(trade: Omit<TradeCreation, "signature">) {
  return {
    checks: {
      uses: BigInt(trade.checks.uses),
      expiration: BigInt(toSeconds(trade.checks.expiration)),
      effective: BigInt(toSeconds(trade.checks.effective)),
      salt: pad32(trade.checks.salt),
      contractSignatureIndex: BigInt(trade.checks.contractSignatureIndex),
      signerSignatureIndex: BigInt(trade.checks.signerSignatureIndex),
      allowedRoot: pad32(trade.checks.allowedRoot || "0x"),
      externalChecks: (trade.checks.externalChecks ?? []).map((check) => ({
        contractAddress: check.contractAddress as Hex,
        selector: check.selector as Hex,
        value: bytes(check.value),
        required: check.required,
      })),
    },
    sent: trade.sent.map((a) => ({
      assetType: BigInt(a.assetType),
      contractAddress: a.contractAddress as Hex,
      value: BigInt(valueForAsset(a)),
      extra: bytes(a.extra),
    })),
    received: trade.received.map((a) => ({
      assetType: BigInt(a.assetType),
      contractAddress: a.contractAddress as Hex,
      value: BigInt(valueForAsset(a)),
      extra: bytes(a.extra),
      beneficiary: a.beneficiary as Hex,
    })),
  };
}

export function tradeDomain(marketplace: ContractData) {
  return { name: marketplace.name, version: marketplace.version, verifyingContract: marketplace.address, salt: chainSalt(marketplace.chainId) };
}

export function tradeTypedData(trade: Omit<TradeCreation, "signature">, marketplace: ContractData) {
  return { domain: tradeDomain(marketplace), types: OFFCHAIN_MARKETPLACE_TYPES, primaryType: "Trade", message: generateTradeValues(trade) };
}

export async function readSignatureIndexes(
  marketplace: ContractData,
  signer: string,
): Promise<{ contractSignatureIndex: number; signerSignatureIndex: number }> {
  const [contractIndex, signerIndex] = await Promise.all([
    readContract({ chainId: marketplace.chainId, address: marketplace.address, abi: MARKETPLACE_ABI, functionName: "contractSignatureIndex" }),
    readContract({
      chainId: marketplace.chainId,
      address: marketplace.address,
      abi: MARKETPLACE_ABI,
      functionName: "signerSignatureIndex",
      args: [signer],
    }),
  ]);
  return { contractSignatureIndex: Number(contractIndex as bigint), signerSignatureIndex: Number(signerIndex as bigint) };
}

export function randomSalt(): Hex {
  const bytes32 = new Uint8Array(32);
  crypto.getRandomValues(bytes32);
  return `0x${[...bytes32].map((b) => b.toString(16).padStart(2, "0")).join("")}`;
}

export function networkOf(chainId: ChainId): TradeNetwork {
  return isPolygon(chainId) ? "MATIC" : "ETHEREUM";
}

export function usdCentsToUsdWei(cents: number): string {
  if (!Number.isSafeInteger(cents) || cents <= 0) throw new RangeError("A price must be a positive whole number of cents");
  return (BigInt(cents) * 10n ** 16n).toString();
}

export type TradeDeps = {
  now?: () => number;
  salt?: () => Hex;
  readIndexes?: typeof readSignatureIndexes;
};

async function draft(
  signer: string,
  chainId: ChainId,
  type: TradeType,
  uses: number,
  expiresAtMs: number,
  sent: TradeAsset[],
  received: TradeReceivedAsset[],
  deps: TradeDeps,
): Promise<{ trade: Omit<TradeCreation, "signature">; marketplace: ContractData }> {
  const marketplace = latestOffChainMarketplace(chainId);
  const indexes = await (deps.readIndexes ?? readSignatureIndexes)(marketplace, signer);
  const now = (deps.now ?? Date.now)();
  if (!(expiresAtMs > now)) throw new RangeError("The expiration must be in the future");
  return {
    marketplace,
    trade: {
      signer,
      network: networkOf(chainId),
      chainId,
      type,
      checks: {
        uses,
        expiration: expiresAtMs,
        effective: now,
        salt: (deps.salt ?? randomSalt)(),
        contractSignatureIndex: indexes.contractSignatureIndex,
        signerSignatureIndex: indexes.signerSignatureIndex,
        allowedRoot: "0x",
        externalChecks: [],
      },
      sent,
      received,
    },
  };
}

async function sign(provider: Eip1193Provider, signer: string, built: Awaited<ReturnType<typeof draft>>): Promise<TradeCreation> {
  const signature = await signTypedData(provider, signer, tradeTypedData(built.trade, built.marketplace));
  return { ...built.trade, signature };
}

export async function createManaListing(
  opts: { provider: Eip1193Provider; nft: { contractAddress: string; tokenId: string; chainId: ChainId }; priceWei: bigint; expiresAtMs: number; fingerprint?: string },
  deps: TradeDeps = {},
): Promise<TradeCreation> {
  if (opts.priceWei <= 0n) throw new RangeError("A listing needs a positive MANA price");
  const seller = await walletAccount(opts.provider);
  const built = await draft(
    seller,
    opts.nft.chainId,
    "public_nft_order",
    1,
    opts.expiresAtMs,
    [{ assetType: TRADE_ASSET.ERC721, contractAddress: opts.nft.contractAddress, tokenId: opts.nft.tokenId, extra: opts.fingerprint ?? "" }],
    [{ assetType: TRADE_ASSET.ERC20, contractAddress: manaToken(opts.nft.chainId).address, amount: opts.priceWei.toString(), extra: "", beneficiary: seller }],
    deps,
  );
  return sign(opts.provider, seller, built);
}

export async function createUsdPeggedListing(
  opts: { provider: Eip1193Provider; nft: { contractAddress: string; tokenId: string; chainId: ChainId }; usdCents: number; expiresAtMs: number; fingerprint?: string },
  deps: TradeDeps = {},
): Promise<TradeCreation> {
  const seller = await walletAccount(opts.provider);
  const built = await draft(
    seller,
    opts.nft.chainId,
    "public_nft_order",
    1,
    opts.expiresAtMs,
    [{ assetType: TRADE_ASSET.ERC721, contractAddress: opts.nft.contractAddress, tokenId: opts.nft.tokenId, extra: opts.fingerprint ?? "" }],
    [
      {
        assetType: TRADE_ASSET.USD_PEGGED_MANA,
        contractAddress: manaToken(opts.nft.chainId).address,
        amount: usdCentsToUsdWei(opts.usdCents),
        extra: "",
        beneficiary: seller,
      },
    ],
    deps,
  );
  return sign(opts.provider, seller, built);
}

export async function createPrimaryUsdPeggedListing(
  opts: { provider: Eip1193Provider; item: { contractAddress: string; itemId: string; chainId: ChainId }; usdCents: number; expiresAtMs: number; uses?: number },
  deps: TradeDeps = {},
): Promise<TradeCreation> {
  const creator = await walletAccount(opts.provider);
  const uses = Math.max(1, Math.floor(opts.uses ?? 1));
  const built = await draft(
    creator,
    opts.item.chainId,
    "public_item_order",
    uses,
    opts.expiresAtMs,
    [{ assetType: TRADE_ASSET.COLLECTION_ITEM, contractAddress: opts.item.contractAddress, itemId: opts.item.itemId, extra: "" }],
    [
      {
        assetType: TRADE_ASSET.USD_PEGGED_MANA,
        contractAddress: manaToken(opts.item.chainId).address,
        amount: usdCentsToUsdWei(opts.usdCents),
        extra: "",
        beneficiary: creator,
      },
    ],
    deps,
  );
  return sign(opts.provider, creator, built);
}

export async function createBidTrade(
  opts: {
    provider: Eip1193Provider;
    asset: { contractAddress: string; chainId: ChainId } & ({ tokenId: string } | { itemId: string });
    priceWei: bigint;
    expiresAtMs: number;
    fingerprint?: string;
  },
  deps: TradeDeps = {},
): Promise<TradeCreation> {
  if (opts.priceWei <= 0n) throw new RangeError("A bid needs a positive MANA amount");
  const bidder = await walletAccount(opts.provider);
  const received: TradeReceivedAsset =
    "tokenId" in opts.asset
      ? { assetType: TRADE_ASSET.ERC721, contractAddress: opts.asset.contractAddress, tokenId: opts.asset.tokenId, extra: opts.fingerprint ?? "", beneficiary: bidder }
      : { assetType: TRADE_ASSET.COLLECTION_ITEM, contractAddress: opts.asset.contractAddress, itemId: opts.asset.itemId, extra: "", beneficiary: bidder };
  const built = await draft(
    bidder,
    opts.asset.chainId,
    "bid",
    1,
    opts.expiresAtMs,
    [{ assetType: TRADE_ASSET.ERC20, contractAddress: manaToken(opts.asset.chainId).address, amount: opts.priceWei.toString(), extra: "" }],
    [received],
    deps,
  );
  return sign(opts.provider, bidder, built);
}

export function onChainTrade(trade: Trade, caller: string) {
  const values = generateTradeValues(trade);
  return {
    signer: trade.signer as Hex,
    signature: trade.signature as Hex,
    checks: {
      uses: values.checks.uses,
      expiration: values.checks.expiration,
      effective: values.checks.effective,
      salt: values.checks.salt,
      contractSignatureIndex: values.checks.contractSignatureIndex,
      signerSignatureIndex: values.checks.signerSignatureIndex,
      allowedRoot: values.checks.allowedRoot,
      allowedProof: (trade.checks.allowedProof ?? []).map((p) => pad32(p)),
      externalChecks: values.checks.externalChecks,
    },
    sent: values.sent.map((a) => ({ assetType: a.assetType, contractAddress: a.contractAddress, value: a.value, beneficiary: caller as Hex, extra: a.extra })),
    received: values.received.map((a) => ({
      assetType: a.assetType,
      contractAddress: a.contractAddress,
      value: a.value,
      beneficiary: a.beneficiary ?? (ZERO_ADDRESS as Hex),
      extra: a.extra,
    })),
  };
}

export class TradeNotFoundError extends Error {
  override name = "TradeNotFoundError";
  constructor(readonly tradeId: string) {
    super("fetchTrade 404");
  }
}

function unwrap<T>(body: unknown): T {
  const b = body as { ok?: boolean; data?: T; message?: string } | null;
  if (b && typeof b === "object" && "ok" in b) {
    if (b.ok === false) throw new Error(b.message ?? "The Shop refused the request");
    return b.data as T;
  }
  return body as T;
}

export async function fetchTrade(tradeId: string): Promise<Trade> {
  try {
    return unwrap<Trade>(await shopClient().market.trade(tradeId));
  } catch (e) {
    if (isShopApiError(e) && e.status === 404) throw new TradeNotFoundError(tradeId);
    throw e;
  }
}

export async function postTrade(trade: TradeCreation): Promise<Trade> {
  const created = unwrap<Trade>(await shopClient().market.createTrade(trade));
  if (!created?.id) throw new Error("The Shop returned a listing without an id");
  return created;
}

export async function hasApproval(opts: { contractAddress: string; chainId: ChainId; owner: string; operator?: string }): Promise<boolean> {
  const operator = opts.operator ?? latestOffChainMarketplace(opts.chainId).address;
  return (await readContract({
    chainId: opts.chainId,
    address: opts.contractAddress,
    abi: ERC721_ABI,
    functionName: "isApprovedForAll",
    args: [opts.owner, operator],
  })) as boolean;
}

export async function ensureApproval(opts: { provider: Eip1193Provider; contractAddress: string; chainId: ChainId; wait?: WaitOptions }): Promise<string | null> {
  const owner = await walletAccount(opts.provider);
  const marketplace = latestOffChainMarketplace(opts.chainId);
  if (await hasApproval({ contractAddress: opts.contractAddress, chainId: opts.chainId, owner, operator: marketplace.address })) return null;
  const data = encodeFunctionData({ abi: ERC721_ABI, functionName: "setApprovalForAll", args: [marketplace.address, true] });
  const hash = isPolygon(opts.chainId)
    ? await sendMetaTransaction({ provider: opts.provider, contract: collectionV2(opts.chainId, opts.contractAddress), kind: "collection", functionData: data })
    : await sendCall(opts.provider, {
        chainId: opts.chainId,
        to: opts.contractAddress,
        abi: ERC721_ABI,
        functionName: "setApprovalForAll",
        args: [marketplace.address, true],
      });
  return waitForReceipt(opts.chainId, hash, "The approval", opts.wait);
}

export async function isMarketplaceMinter(opts: { contractAddress: string; chainId: ChainId }): Promise<boolean> {
  const marketplace = latestOffChainMarketplace(opts.chainId);
  return (await readContract({
    chainId: opts.chainId,
    address: opts.contractAddress,
    abi: COLLECTION_V2_ABI,
    functionName: "globalMinters",
    args: [marketplace.address],
  })) as boolean;
}

export async function ensureMinter(opts: { provider: Eip1193Provider; contractAddress: string; chainId: ChainId; wait?: WaitOptions }): Promise<string | null> {
  if (await isMarketplaceMinter(opts)) return null;
  const marketplace = latestOffChainMarketplace(opts.chainId);
  const data = encodeFunctionData({ abi: COLLECTION_V2_ABI, functionName: "setMinters", args: [[marketplace.address], [true]] });
  const hash = await sendMetaTransaction({ provider: opts.provider, contract: collectionV2(opts.chainId, opts.contractAddress), kind: "collection", functionData: data });
  return waitForReceipt(opts.chainId, hash, "Enabling sales for this collection", opts.wait);
}

export type CancelMode = "auto" | "gasless-only" | "direct";

export class GaslessCancelFailedError extends Error {
  override name = "GaslessCancelFailedError";
  constructor(readonly original: unknown) {
    super("The gasless cancellation was not confirmed");
  }
}

function tradeMarketplace(trade: Trade): ContractData {
  const found = marketplaceForTrade({ contract: trade.contract, chainId: trade.chainId });
  if (!found) throw new Error(`The listing names ${trade.contract}, which is not a marketplace on chain ${trade.chainId}`);
  return found;
}

export async function cancelTrade(opts: { provider: Eip1193Provider; trade: Trade; mode?: CancelMode; wait?: WaitOptions }): Promise<string> {
  const { provider, trade, mode = "auto" } = opts;
  const signer = await walletAccount(provider);
  if (signer !== trade.signer.toLowerCase()) throw new Error("Only the account that signed this can cancel it.");
  const marketplace = tradeMarketplace(trade);
  const args = [[onChainTrade(trade, signer)]] as const;
  if (isPolygon(trade.chainId) && mode !== "direct") {
    try {
      const data = encodeFunctionData({ abi: MARKETPLACE_ABI, functionName: "cancelSignature", args });
      const hash = await sendMetaTransaction({ provider, contract: marketplace, kind: "offchain", functionData: data });
      return await waitForReceipt(trade.chainId, hash, "The cancellation", opts.wait);
    } catch (e) {
      if (declined(e)) throw e;
      if (mode === "gasless-only") throw new GaslessCancelFailedError(e);
      if (!(e instanceof MetaTransactionError)) throw e;
    }
  }
  await requireChain(provider, trade.chainId);
  const hash = await sendCall(provider, { chainId: trade.chainId, to: marketplace.address, abi: MARKETPLACE_ABI, functionName: "cancelSignature", args });
  return waitForReceipt(trade.chainId, hash, "The cancellation", opts.wait);
}

export const cancelListing = cancelTrade;

export async function acceptTrade(opts: { provider: Eip1193Provider; trade: Trade; wait?: WaitOptions; onSubmitted?: (hash: string) => void }): Promise<string> {
  const caller = await walletAccount(opts.provider);
  const marketplace = tradeMarketplace(opts.trade);
  const hash = await sendCall(opts.provider, {
    chainId: opts.trade.chainId,
    to: marketplace.address,
    abi: MARKETPLACE_ABI,
    functionName: "accept",
    args: [[onChainTrade(opts.trade, caller)]],
  });
  opts.onSubmitted?.(hash);
  return waitForReceipt(opts.trade.chainId, hash, "The trade", opts.wait);
}

export function tradePrice(trade: Pick<TradeCreation, "received" | "sent" | "type">): { kind: "mana"; wei: bigint } | { kind: "usdPegged"; usdWei: bigint } | null {
  const leg = trade.type === "bid" ? trade.sent[0] : trade.received[0];
  if (!leg || !("amount" in leg)) return null;
  if (leg.assetType === TRADE_ASSET.USD_PEGGED_MANA) return { kind: "usdPegged", usdWei: BigInt(leg.amount) };
  return { kind: "mana", wei: BigInt(leg.amount) };
}

export function isResaleTrade(trade: Pick<TradeCreation, "sent" | "type">): boolean {
  return trade.type === "public_nft_order" || trade.sent.some((a) => a.assetType === TRADE_ASSET.ERC721);
}

