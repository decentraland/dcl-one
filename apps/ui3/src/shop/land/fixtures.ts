// LAND, estate, NAME and rental fixtures for tests and stories, shaped as the v2 market routes answer
// (the generated NftResult / RentalListing types) with mainnet contracts and synthetic owners.

import { landTokenId } from "../../data/catalyst/landListing";
import type { NftResult } from "@catalyst-api/market/NftResult";
import type { RentalListing } from "@catalyst-api/market/RentalListing";
import { CHAIN } from "../model/chain";
import { LAND_NETWORKS } from "./contracts";
import { coordsId, type LandTile, type Tiles } from "./tiles";

export const NET = LAND_NETWORKS[CHAIN.ETHEREUM]!;
export const OWNER = "0x0000000000000000000000000000000000000a11";
export const TENANT = "0x0000000000000000000000000000000000000b0b";
export const DAY = 86_400_000;
const MANA = 10n ** 18n;

export function parcelResult(over: { x?: number; y?: number; owner?: string; order?: NftResult["order"]; rental?: RentalListing | null; estate?: { name: string; tokenId: string } | null; name?: string } = {}): NftResult {
  const x = over.x ?? 12;
  const y = over.y ?? -4;
  const tokenId = landTokenId(x, y);
  return {
    nft: {
      activeOrderId: over.order ? over.order.id : null,
      category: "parcel",
      chainId: CHAIN.ETHEREUM,
      contractAddress: NET.land,
      createdAt: 0,
      data: { parcel: { x: String(x), y: String(y), description: "A parcel near the plaza", estate: over.estate ?? null } },
      id: `${NET.land}-${tokenId}`,
      image: "",
      issuedId: null,
      itemId: null,
      name: over.name ?? `Parcel ${x},${y}`,
      network: "ETHEREUM",
      openRentalId: over.rental?.status === "open" ? over.rental.id : (over.rental?.id ?? null),
      owner: over.owner ?? OWNER,
      tokenId,
      soldAt: 0,
      updatedAt: 0,
      url: "",
    },
    order: over.order ?? null,
    rental: over.rental ?? null,
  };
}

export function estateResult(over: { id?: string; owner?: string; parcels?: { x: number; y: number }[]; rental?: RentalListing | null; order?: NftResult["order"] } = {}): NftResult {
  const parcels = over.parcels ?? [
    { x: 10, y: 10 },
    { x: 11, y: 10 },
    { x: 10, y: 11 },
  ];
  const id = over.id ?? "4213";
  return {
    nft: {
      activeOrderId: null,
      category: "estate",
      chainId: CHAIN.ETHEREUM,
      contractAddress: NET.estate,
      createdAt: 0,
      data: { estate: { size: parcels.length, description: "Three parcels by the road", parcels } },
      id: `${NET.estate}-${id}`,
      image: "",
      issuedId: null,
      itemId: null,
      name: "Corner Estate",
      network: "ETHEREUM",
      openRentalId: over.rental?.id ?? null,
      owner: over.owner ?? OWNER,
      tokenId: id,
      soldAt: 0,
      updatedAt: 0,
      url: "",
    },
    order: over.order ?? null,
    rental: over.rental ?? null,
  };
}

export function nameResult(name = "Nacho", over: { price?: string | null; tokenId?: string } = {}): NftResult {
  const tokenId = over.tokenId ?? "1";
  return {
    nft: {
      activeOrderId: over.price ? "o-name" : null,
      category: "ens",
      chainId: CHAIN.ETHEREUM,
      contractAddress: NET.nameRegistrar,
      createdAt: 0,
      data: { ens: { subdomain: name } },
      id: `${NET.nameRegistrar}-${tokenId}`,
      image: "",
      issuedId: null,
      itemId: null,
      name,
      network: "ETHEREUM",
      openRentalId: null,
      owner: OWNER,
      tokenId,
      soldAt: 0,
      updatedAt: 0,
      url: "",
    },
    order: over.price ? saleOrder(over.price, NET.nameRegistrar, tokenId) : null,
    rental: null,
  };
}

export function saleOrder(priceWei: string, contractAddress: string = NET.land, tokenId = "1"): NonNullable<NftResult["order"]> {
  return {
    id: "order-1",
    marketplaceAddress: "0x8e5660b4ab70168b5a6feea0e0315cb49c8cd539",
    contractAddress,
    tokenId,
    owner: OWNER,
    buyer: null,
    price: priceWei,
    status: "open",
    expiresAt: 1893456000,
    createdAt: 1790000000,
    updatedAt: 1790000000,
    network: "ETHEREUM",
    chainId: CHAIN.ETHEREUM,
    issuedId: null,
    tradeId: null,
  };
}

export function rentalListing(over: Partial<RentalListing> = {}): RentalListing {
  return {
    id: "rental-1",
    nftId: `${NET.land}-${landTokenId(12, -4)}`,
    category: "parcel",
    searchText: "12,-4",
    network: "ETHEREUM",
    chainId: CHAIN.ETHEREUM,
    expiration: Date.UTC(2030, 0, 1),
    signature: `0x${"ab".repeat(64)}1c`,
    nonces: ["0", "0", "0"],
    tokenId: landTokenId(12, -4),
    contractAddress: NET.land,
    rentalContractAddress: NET.rentals,
    lessor: OWNER,
    tenant: null,
    status: "open",
    createdAt: 0,
    updatedAt: 0,
    startedAt: null,
    periods: [
      { minDays: 7, maxDays: 7, pricePerDay: (2n * MANA).toString() },
      { minDays: 30, maxDays: 30, pricePerDay: MANA.toString() },
    ],
    target: "0x0000000000000000000000000000000000000000",
    rentedDays: null,
    ...over,
  };
}

/** A small grid around the origin: a road row, a plaza, owned parcels, some on sale or for rent. */
export function sampleTiles(): Tiles {
  const tiles: Tiles = {};
  const put = (x: number, y: number, t: Partial<LandTile>) => {
    tiles[coordsId(x, y)] = { id: coordsId(x, y), x, y, kind: "owned", top: false, left: false, topLeft: false, owner: OWNER, name: null, estateId: null, price: null, rentalPricePerDay: null, ...t };
  };
  for (let x = -20; x <= 20; x++) put(x, 0, { kind: "road", owner: null });
  for (let x = -3; x <= 3; x++) for (let y = -3; y <= -1; y++) put(x, y, { kind: "plaza", owner: null });
  for (let x = 5; x <= 15; x++) for (let y = -6; y <= -2; y++) put(x, y, {});
  put(12, -4, { price: 4000, name: "Near the plaza" });
  put(13, -4, { rentalPricePerDay: (2n * MANA).toString() });
  for (const p of [
    { x: 10, y: 10 },
    { x: 11, y: 10 },
    { x: 10, y: 11 },
  ])
    put(p.x, p.y, { estateId: "4213", name: "Corner Estate" });
  return tiles;
}
