// from: marketplace@f3956826:webapp/src/modules/rental/{utils,sagas,types,actions}.ts,
// webapp/src/modules/vendor/decentraland/rentals/{api,utils}.ts
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
// LAND and estate rentals: UPSERT_RENTAL (sign a listing and post it), CLAIM_ASSET (take the LAND back
// after a rental), REMOVE_RENTAL (bump the asset nonce so the signed listing dies on-chain) and
// ACCEPT_RENTAL_LISTING (rent: MANA allowance, then `acceptListing`). Rentals are MANA only (SHOP.md):
// nothing here touches credits, quotes a credit price or offers a top-up. The sagas became plain async
// functions over the wallet seam; the signatures server is reached through the one Shop client
// (`/shop-api/v1/rentals-listings`, legacy `/signatures-api/v1/...`).

import { parseAbi, parseUnits, type Hex } from "viem";

import type { Eip1193Provider } from "../../data/auth/wallet";
import type { RentalListing } from "@catalyst-api/market/RentalListing";
import type { RentalListingPeriod } from "@catalyst-api/market/RentalListingPeriod";
import { shopClient, type ShopClient } from "../api/shop";
import { requireChain } from "../wallet/chain";
import { landCategoryOf, landNetwork, RENTALS_DOMAIN } from "./contracts";
import { readEstateFingerprint, ZERO_FINGERPRINT } from "./estate";
import {
  encodeCall,
  ensureAllowance,
  readContract,
  rpcReader,
  sendTransaction,
  waitForReceipt,
  walletReader,
  type ApprovalStage,
  type ChainReader,
} from "./evm";

export type { RentalListing, RentalListingPeriod };

export type RentalStatus = "open" | "cancelled" | "executed" | "claimed";

export enum PeriodOption {
  ONE_DAY = "one_day",
  ONE_WEEK = "one_week",
  ONE_MONTH = "one_month",
  TWO_MONTHS = "two_months",
  QUARTER_YEAR = "quarter_year",
  HALF_YEAR = "half_year",
  ONE_YEAR = "one_year",
}

export const daysByPeriod: Record<PeriodOption, number> = {
  [PeriodOption.ONE_DAY]: 1,
  [PeriodOption.ONE_WEEK]: 7,
  [PeriodOption.ONE_MONTH]: 30,
  [PeriodOption.TWO_MONTHS]: 60,
  [PeriodOption.QUARTER_YEAR]: 90,
  [PeriodOption.HALF_YEAR]: 180,
  [PeriodOption.ONE_YEAR]: 365,
};

export const PERIOD_OPTIONS: readonly PeriodOption[] = Object.values(PeriodOption);

export const periodsByDays: Record<number, PeriodOption> = Object.fromEntries(
  PERIOD_OPTIONS.map((p) => [daysByPeriod[p], p]),
) as Record<number, PeriodOption>;

export const PERIOD_LABEL: Record<PeriodOption, string> = {
  [PeriodOption.ONE_DAY]: "1 day",
  [PeriodOption.ONE_WEEK]: "7 days",
  [PeriodOption.ONE_MONTH]: "30 days",
  [PeriodOption.TWO_MONTHS]: "60 days",
  [PeriodOption.QUARTER_YEAR]: "90 days",
  [PeriodOption.HALF_YEAR]: "180 days",
  [PeriodOption.ONE_YEAR]: "365 days",
};

/** Upstream's `RENTAL_MIN_PRICE`: a day of LAND costs at least one MANA. */
export const RENTAL_MIN_PRICE_MANA = 1;
const DAY_MS = 86_400_000;
const ZERO_ADDRESS = "0x0000000000000000000000000000000000000000";

export const rentalsAbi = parseAbi([
  "function getAssetIndex(address _contractAddress, uint256 _tokenId, address _signer) view returns (uint256)",
  "function getSignerIndex(address _signer) view returns (uint256)",
  "function getContractIndex() view returns (uint256)",
  "function claim(address[] _contractAddresses, uint256[] _tokenIds)",
  "function bumpAssetIndex(address _contractAddress, uint256 _tokenId)",
  "function acceptListing((address signer, address contractAddress, uint256 tokenId, uint256 expiration, uint256[3] indexes, uint256[] pricePerDay, uint256[] maxDays, uint256[] minDays, address target, bytes signature) _listing, address _operator, uint256 _conditionIndex, uint256 _rentalDays, bytes32 _fingerprint)",
]);

export function maxPriceOfPeriods(rental: { periods: readonly RentalListingPeriod[] }): string {
  return rental.periods.reduce((max, p) => (BigInt(max) >= BigInt(p.pricePerDay) ? max : p.pricePerDay), "0");
}

/** The MANA a period costs in total: its price per day times its days. */
export function pricePerRent(period: RentalListingPeriod): bigint {
  return BigInt(period.pricePerDay) * BigInt(period.maxDays);
}

export function sortPeriods(periods: readonly RentalListingPeriod[]): RentalListingPeriod[] {
  return [...periods].sort((a, b) => a.minDays - b.minDays);
}

export const isRentalOpen = (r: RentalListing | null | undefined): boolean => !!r && r.status === "open";
export const isRentalExecuted = (r: RentalListing | null | undefined): boolean => !!r && r.status === "executed";
export const isRentalCancelled = (r: RentalListing | null | undefined): boolean => !!r && r.status === "cancelled";

/** When a started rental ends; null before it starts. `startedAt` is epoch ms. */
export function rentalEndDate(rental: RentalListing): Date | null {
  return rental.startedAt && rental.rentedDays ? new Date(rental.startedAt + rental.rentedDays * DAY_MS) : null;
}

export function hasRentalEnded(rental: RentalListing, now: number = Date.now()): boolean {
  const end = rentalEndDate(rental);
  return end ? end.getTime() <= now : false;
}

export function canCreateANewRental(rental: RentalListing | null, now: number = Date.now()): boolean {
  return rental === null || isRentalCancelled(rental) || (isRentalExecuted(rental) && hasRentalEnded(rental, now));
}

const same = (a: string | null | undefined, b: string | null | undefined) => !!a && !!b && a.toLowerCase() === b.toLowerCase();

export type RentalAsset = { chainId: number; contractAddress: string; tokenId: string; owner: string };

/**
 * The lessor can take the LAND back: an executed rental whose time is over, or an open/cancelled
 * listing whose asset still sits in the Rentals contract from a past rental.
 */
export function canBeClaimed(userAddress: string, rental: RentalListing, asset: RentalAsset, now: number = Date.now()): boolean {
  if (same(userAddress, asset.owner)) return false;
  if (rental.status === "executed") return hasRentalEnded(rental, now);
  if ((isRentalOpen(rental) || isRentalCancelled(rental)) && same(userAddress, rental.lessor)) {
    return same(landNetwork(asset.chainId).rentals, asset.owner);
  }
  return false;
}

/** Transfers, sales and bids are locked while rented or while the LAND waits to be claimed back. */
export function isLandLocked(userAddress: string, rental: RentalListing, asset: RentalAsset, now: number = Date.now()): boolean {
  return rental.status === "executed" || canBeClaimed(userAddress, rental, asset, now);
}

/** The owner of a LAND in a rental is its lessor while the Rentals contract holds it. */
export function isOwnedBy(asset: RentalAsset, address: string | null | undefined, rental?: RentalListing | null): boolean {
  if (!address) return false;
  if (same(asset.owner, address)) return true;
  return !!rental && same(rental.lessor, address) && same(landNetwork(asset.chainId).rentals, asset.owner);
}

/** Some wallets sign with v = 0/1; the contract wants 27/28. */
export function generateECDSASignatureWithValidV(signature: string): string {
  const last = Number.parseInt(signature.slice(-2), 16);
  return last === 27 || last === 28 ? signature : signature.slice(0, -2) + (last + 27).toString(16);
}

// ---- the signatures server, through the one Shop client ----

type Envelope<T> = { ok?: boolean; data?: T; message?: string; error?: string };

function unwrap<T>(body: unknown): T {
  const env = body as Envelope<T> | null;
  if (env && typeof env === "object" && "ok" in env) {
    if (env.ok && env.data !== undefined) return env.data;
    throw new Error(env.message ?? env.error ?? "The rentals service refused the request.");
  }
  return body as T;
}

export type RentalListingsQuery = {
  nftIds?: string[];
  lessor?: string;
  tenant?: string;
  tokenId?: string;
  contractAddresses?: string[];
  status?: RentalStatus[];
  category?: "parcel" | "estate";
  rentalDays?: number[];
  sortBy?: string;
  sortDirection?: "asc" | "desc";
  page?: number;
  limit?: number;
};

export async function fetchRentalListings(
  query: RentalListingsQuery,
  opts: { client?: ShopClient; signal?: AbortSignal } = {},
): Promise<{ results: RentalListing[]; total: number }> {
  const client = opts.client ?? shopClient();
  const body = await client.rentals.listings<unknown>(query as never, { signal: opts.signal });
  const data = unwrap<{ results?: RentalListing[]; total?: number }>(body);
  return { results: data.results ?? [], total: data.total ?? 0 };
}

export async function refreshRentalListing(id: string, opts: { client?: ShopClient } = {}): Promise<RentalListing> {
  const client = opts.client ?? shopClient();
  return unwrap<RentalListing>(await client.rentals.update<unknown>(id, undefined));
}

export async function fetchRentalListingPrices(
  filters: Omit<RentalListingsQuery, "rentalDays"> & { rentalDays?: number[] },
  opts: { client?: ShopClient; signal?: AbortSignal } = {},
): Promise<Record<string, number>> {
  const client = opts.client ?? shopClient();
  return unwrap<Record<string, number>>(await client.rentals.prices<unknown>(filters as never, { signal: opts.signal }));
}

export type RentalListingCreation = {
  network: "ETHEREUM";
  chainId: number;
  /** Epoch ms. */
  expiration: number;
  contractAddress: string;
  tokenId: string;
  nonces: string[];
  periods: { minDays: number; maxDays: number; pricePerDay: string }[];
  rentalContractAddress: string;
  signature: string;
  target: string;
};

// ---- chain reads ----

/** [contract, signer, asset] nonces the listing signs over (upstream `getNonces`). */
export async function readRentalNonces(reader: ChainReader, asset: { contractAddress: string; tokenId: string }, signer: string): Promise<[string, string, string]> {
  const rentals = landNetwork(reader.chainId).rentals;
  const [contract, signerIndex, assetIndex] = await Promise.all([
    readContract<bigint>(reader, rentals, rentalsAbi, "getContractIndex", []),
    readContract<bigint>(reader, rentals, rentalsAbi, "getSignerIndex", [signer]),
    readContract<bigint>(reader, rentals, rentalsAbi, "getAssetIndex", [asset.contractAddress, BigInt(asset.tokenId), signer]),
  ]);
  return [contract.toString(), signerIndex.toString(), assetIndex.toString()];
}

/** The EIP-712 listing a lessor signs; the signatures server and the Rentals contract verify it. */
export function rentalTypedData(
  chainId: number,
  listing: { signer: string; contractAddress: string; tokenId: string; expiration: number; nonces: readonly string[]; periods: RentalListingCreation["periods"] },
) {
  return {
    domain: { ...RENTALS_DOMAIN, chainId, verifyingContract: landNetwork(chainId).rentals },
    types: {
      EIP712Domain: [
        { name: "name", type: "string" },
        { name: "version", type: "string" },
        { name: "chainId", type: "uint256" },
        { name: "verifyingContract", type: "address" },
      ],
      Listing: [
        { name: "signer", type: "address" },
        { name: "contractAddress", type: "address" },
        { name: "tokenId", type: "uint256" },
        { name: "expiration", type: "uint256" },
        { name: "indexes", type: "uint256[3]" },
        { name: "pricePerDay", type: "uint256[]" },
        { name: "maxDays", type: "uint256[]" },
        { name: "minDays", type: "uint256[]" },
        { name: "target", type: "address" },
      ],
    },
    primaryType: "Listing" as const,
    message: {
      signer: listing.signer.toLowerCase(),
      contractAddress: listing.contractAddress.toLowerCase(),
      tokenId: listing.tokenId,
      expiration: Math.trunc(listing.expiration / 1000).toString(),
      indexes: [...listing.nonces],
      pricePerDay: listing.periods.map((p) => p.pricePerDay),
      maxDays: listing.periods.map((p) => String(p.maxDays)),
      minDays: listing.periods.map((p) => String(p.minDays)),
      target: ZERO_ADDRESS,
    },
  };
}

// ---- actions ----

export type RentalAssetRef = { chainId: number; contractAddress: string; tokenId: string; openRentalId?: string | null };

export type UpsertRentalInput = {
  provider: Eip1193Provider;
  account: string;
  asset: RentalAssetRef;
  /** MANA per day as typed, e.g. "12.5". */
  pricePerDay: string;
  periods: readonly PeriodOption[];
  /** Epoch ms. */
  expiresAt: number;
  client?: ShopClient;
  reader?: ChainReader;
};

export class RentalInputError extends Error {
  override name = "RentalInputError";
}

/** The periods a listing publishes: one fixed-length period per chosen option, all at one price. */
export function listingPeriods(pricePerDay: string, periods: readonly PeriodOption[]): RentalListingCreation["periods"] {
  let wei: bigint;
  try {
    wei = parseUnits(pricePerDay.trim(), 18);
  } catch {
    throw new RentalInputError("Invalid price");
  }
  if (wei < parseUnits(String(RENTAL_MIN_PRICE_MANA), 18)) throw new RentalInputError("The price per day cannot be less than 1 MANA");
  if (periods.length === 0) throw new RentalInputError("Choose at least one rent period");
  return [...periods]
    .sort((a, b) => daysByPeriod[a] - daysByPeriod[b])
    .map((p) => ({ minDays: daysByPeriod[p], maxDays: daysByPeriod[p], pricePerDay: wei.toString() }));
}

/** UPSERT_RENTAL_REQUEST: read the nonces, sign the listing in the wallet, post it signed. */
export async function upsertRental(input: UpsertRentalInput): Promise<RentalListing> {
  const { provider, asset } = input;
  if (!landCategoryOf(asset.contractAddress)) throw new RentalInputError("Only LAND and estates can be rented");
  if (input.expiresAt <= Date.now()) throw new RentalInputError("Invalid date");
  const periods = listingPeriods(input.pricePerDay, input.periods);
  await requireChain(provider, asset.chainId);
  const reader = input.reader ?? walletReader(provider, asset.chainId);
  const nonces = await readRentalNonces(reader, asset, input.account);
  const typed = rentalTypedData(asset.chainId, { signer: input.account, contractAddress: asset.contractAddress, tokenId: asset.tokenId, expiration: input.expiresAt, nonces, periods });
  const raw = await provider.request({ method: "eth_signTypedData_v4", params: [input.account, JSON.stringify(typed)] });
  if (typeof raw !== "string" || !raw.startsWith("0x")) throw new Error("The wallet returned no signature.");
  const creation: RentalListingCreation = {
    network: "ETHEREUM",
    chainId: asset.chainId,
    expiration: input.expiresAt,
    contractAddress: asset.contractAddress.toLowerCase(),
    tokenId: asset.tokenId,
    nonces,
    periods,
    rentalContractAddress: landNetwork(asset.chainId).rentals,
    signature: generateECDSASignatureWithValidV(raw),
    target: ZERO_ADDRESS,
  };
  const client = input.client ?? shopClient();
  return unwrap<RentalListing>(await client.rentals.create<unknown>(creation));
}

export type RentalTxStage = ApprovalStage | "awaiting-confirmation" | "confirming" | "indexing";

type TxInput = {
  provider: Eip1193Provider;
  account: string;
  asset: RentalAssetRef;
  client?: ShopClient;
  onStage?(stage: RentalTxStage, txHash?: string): void;
  onSubmitted?(txHash: string): void;
  poll?: { retries?: number; delayMs?: number };
  wait?: Parameters<typeof waitForReceipt>[2];
};

/** Polls the listing until it reaches `status` (bounded, as upstream's MAX_REFRESH_SIGNATURES_RETRIES). */
export async function waitUntilRentalChangesStatus(
  rentalId: string,
  status: RentalStatus,
  opts: { client?: ShopClient; retries?: number; delayMs?: number } = {},
): Promise<RentalListing> {
  const retries = opts.retries ?? 60;
  for (let i = 0; i < retries; i++) {
    await new Promise((resolve) => setTimeout(resolve, opts.delayMs ?? 5000));
    try {
      const listing = await refreshRentalListing(rentalId, { client: opts.client });
      if (listing.status === status) return listing;
    } catch {
      // The server may not have seen the block yet: keep polling within the bound.
    }
  }
  throw new RentalIndexLagError(status);
}

/** The transaction confirmed but the rentals index has not caught up within the wait. */
export class RentalIndexLagError extends Error {
  override name = "RentalIndexLagError";
  constructor(readonly status: RentalStatus) {
    super(`The rental did not show as ${status} in time. The transaction confirmed; the listing will update shortly.`);
  }
}

async function send(input: TxInput, data: Hex): Promise<string> {
  const rentals = landNetwork(input.asset.chainId).rentals;
  input.onStage?.("awaiting-confirmation");
  const hash = await sendTransaction(input.provider, { chainId: input.asset.chainId, from: input.account, to: rentals, data });
  input.onSubmitted?.(hash);
  input.onStage?.("confirming", hash);
  await waitForReceipt(input.provider, hash, input.wait);
  return hash;
}

/** CLAIM_ASSET_REQUEST: take the LAND back from the Rentals contract. */
export async function claimAsset(input: TxInput & { rental: RentalListing }): Promise<{ txHash: string; rental: RentalListing }> {
  const { asset } = input;
  const hash = await send(input, encodeCall(rentalsAbi, "claim", [[asset.contractAddress], [BigInt(asset.tokenId)]]));
  input.onStage?.("indexing", hash);
  const rental = await waitUntilRentalChangesStatus(input.rental.id, "claimed", { client: input.client, ...input.poll });
  return { txHash: hash, rental };
}

/** REMOVE_RENTAL_REQUEST: bump the asset nonce so the signed listing can no longer be accepted. */
export async function removeRental(input: TxInput): Promise<{ txHash: string; rental: RentalListing }> {
  const { asset } = input;
  if (!asset.openRentalId) throw new Error("The provided NFT does not have an open rental");
  const hash = await send(input, encodeCall(rentalsAbi, "bumpAssetIndex", [asset.contractAddress, BigInt(asset.tokenId)]));
  input.onStage?.("indexing", hash);
  const rental = await waitUntilRentalChangesStatus(asset.openRentalId, "cancelled", { client: input.client, ...input.poll });
  return { txHash: hash, rental };
}

export type AcceptRentalInput = TxInput & {
  rental: RentalListing;
  periodIndex: number;
  /** Who gets the LAND's operator permission for the rental; the tenant by default. */
  operator: string;
  reader?: ChainReader;
  /** The MANA the buyer agreed to on screen; the rent is refused if the listing asks for more. */
  agreedWei: bigint;
};

/** The tuple `acceptListing` takes, from a listing as the signatures server stores it. */
export function listingTuple(rental: RentalListing) {
  return {
    signer: rental.lessor ?? ZERO_ADDRESS,
    contractAddress: rental.contractAddress,
    tokenId: BigInt(rental.tokenId),
    expiration: BigInt(Math.trunc(rental.expiration / 1000)),
    indexes: rental.nonces.map((n) => BigInt(n)) as unknown as readonly [bigint, bigint, bigint],
    pricePerDay: rental.periods.map((p) => BigInt(p.pricePerDay)),
    maxDays: rental.periods.map((p) => BigInt(p.maxDays)),
    minDays: rental.periods.map((p) => BigInt(p.minDays)),
    target: ZERO_ADDRESS,
    signature: generateECDSASignatureWithValidV(rental.signature) as Hex,
  };
}

/**
 * ACCEPT_RENTAL_LISTING_REQUEST: rent a LAND or an estate for one of its periods, paying MANA on
 * Ethereum. Asks for a MANA allowance to the Rentals contract only when the current one is short,
 * binds an estate rental to the registry's current fingerprint, and waits for the index to show the
 * rental as executed.
 */
export async function acceptRentalListing(input: AcceptRentalInput): Promise<{ txHash: string; rental: RentalListing }> {
  const { provider, asset, rental } = input;
  if (!asset.openRentalId) throw new Error("The provided NFT does not have an open rental");
  const period = rental.periods[input.periodIndex];
  if (!period) throw new RentalInputError("Select a rental period");
  const cost = pricePerRent(period);
  if (cost > input.agreedWei) throw new RentalInputError("The rental price changed. Review it before renting.");
  const network = landNetwork(asset.chainId);
  const category = landCategoryOf(asset.contractAddress);
  if (!category) throw new RentalInputError("Only LAND and estates can be rented");
  await requireChain(provider, asset.chainId);
  await ensureAllowance(provider, {
    chainId: asset.chainId,
    token: network.mana,
    owner: input.account,
    spender: network.rentals,
    required: cost,
    reader: input.reader,
    onStage: input.onStage,
    wait: input.wait,
  });
  const fingerprint =
    category.category === "estate"
      ? await readEstateFingerprint(input.reader ?? rpcReader(asset.chainId), asset.contractAddress, asset.tokenId)
      : ZERO_FINGERPRINT;
  const data = encodeCall(rentalsAbi, "acceptListing", [listingTuple(rental), input.operator, BigInt(input.periodIndex), BigInt(period.maxDays), fingerprint]);
  const hash = await send(input, data);
  input.onStage?.("indexing", hash);
  const updated = await waitUntilRentalChangesStatus(asset.openRentalId, "executed", { client: input.client, ...input.poll });
  return { txHash: hash, rental: updated };
}
