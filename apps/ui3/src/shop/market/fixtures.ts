// Wire fixtures for the marketplace-feature pages' tests and stories, in the shapes v2 serves today (the
// frozen generated types in library/catalyst-api/market) and with the identities the payment
// fixtures use (payment/fixtures.ts BOOK: the live Polygon collection store and marketplace, synthetic
// accounts). Prices are MANA wei; nothing here carries a credit price for a resale.

import type { Account } from "@catalyst-api/market/Account";
import type { Bid } from "@catalyst-api/market/Bid";
import type { Collection } from "@catalyst-api/market/Collection";
import type { FavoriteList } from "@catalyst-api/market/FavoriteList";
import type { NftResult } from "@catalyst-api/market/NftResult";
import type { Sale } from "@catalyst-api/market/Sale";
import { CHAIN } from "../model/chain";
import { BOOK } from "../payment/fixtures";
import { DEFAULT_LIST_ID } from "./lib/favorites";
import type { CollectorRank, CreatorRank, ItemRank, VolumeData } from "./lib/api";

export const MANA = (whole: number | string) => (BigInt(Math.round(Number(whole) * 1000)) * 10n ** 15n).toString();
const DAY = 86_400_000;
import { FIXTURE_NOW } from "./fixtureNow";
export { FIXTURE_NOW };

const thumb = (n: number) => `https://peer.decentraland.org/lambdas/collections/contents/urn:decentraland:matic:collections-v2:${BOOK.collection}:${n}/thumbnail`;

export function nftRow(opts: {
  tokenId: string;
  itemId?: string | null;
  name: string;
  category?: "wearable" | "emote" | "ens" | "parcel" | "estate";
  owner?: string;
  onSale?: { price: string; tradeId?: string | null } | null;
  contract?: string;
  chainId?: number;
  rarity?: string;
}): NftResult {
  const category = opts.category ?? "wearable";
  const chainId = opts.chainId ?? (category === "wearable" || category === "emote" ? CHAIN.POLYGON : CHAIN.ETHEREUM);
  const network = chainId === CHAIN.POLYGON ? "MATIC" : "ETHEREUM";
  const contract = opts.contract ?? (category === "ens" ? BOOK.registrar.contract : category === "parcel" ? BOOK.land : category === "estate" ? BOOK.estate : BOOK.collection);
  const data: NftResult["nft"]["data"] =
    category === "wearable"
      ? { wearable: { bodyShapes: ["BaseMale", "BaseFemale"], category: "upper_body", description: "", rarity: opts.rarity ?? "rare", isSmart: false } }
      : category === "emote"
        ? { emote: { bodyShapes: ["BaseMale"], category: "dance", description: "", rarity: opts.rarity ?? "epic", loop: true, hasSound: false, hasGeometry: false, outcomeType: null } }
        : category === "ens"
          ? { ens: { subdomain: opts.name } }
          : category === "parcel"
            ? { parcel: { x: 12, y: -4, description: null, estate: null } as never }
            : { estate: { size: 4, parcels: [], description: null } as never };
  return {
    nft: {
      activeOrderId: opts.onSale ? `order-${opts.tokenId}` : null,
      category,
      chainId,
      contractAddress: contract,
      createdAt: FIXTURE_NOW - 40 * DAY,
      data,
      id: `${contract}-${opts.tokenId}`,
      image: category === "wearable" || category === "emote" ? thumb(Number(opts.itemId ?? 1)) : "",
      issuedId: opts.itemId ? "12" : null,
      itemId: opts.itemId ?? null,
      name: opts.name,
      network,
      openRentalId: null,
      owner: opts.owner ?? BOOK.buyer,
      tokenId: opts.tokenId,
      soldAt: 0,
      updatedAt: FIXTURE_NOW - DAY,
      url: `/contracts/${contract}/tokens/${opts.tokenId}`,
    },
    order: opts.onSale
      ? {
          id: `order-${opts.tokenId}`,
          marketplaceAddress: BOOK.marketplace.contract,
          contractAddress: contract,
          tokenId: opts.tokenId,
          owner: opts.owner ?? BOOK.buyer,
          buyer: null,
          price: opts.onSale.price,
          status: "open",
          expiresAt: FIXTURE_NOW + 30 * DAY,
          createdAt: FIXTURE_NOW - 2 * DAY,
          updatedAt: FIXTURE_NOW - 2 * DAY,
          network,
          chainId,
          issuedId: "12",
          tradeId: opts.onSale.tradeId ?? `trade-${opts.tokenId}`,
        }
      : null,
    rental: null,
  };
}

export const OWNED_WEARABLES: NftResult[] = [
  nftRow({ tokenId: "105312", itemId: "7", name: "Neon Jacket", onSale: { price: MANA("12.5") } }),
  nftRow({ tokenId: "105313", itemId: "8", name: "Cyber Boots" }),
  nftRow({ tokenId: "105314", itemId: "9", name: "Glow Visor", rarity: "legendary" }),
];

export const OWNED_EMOTES: NftResult[] = [nftRow({ tokenId: "208", itemId: "3", name: "Moonwalk", category: "emote" })];

export const OWNED_NAMES: NftResult[] = [nftRow({ tokenId: "42", name: "Nacho", category: "ens", onSale: { price: MANA(500), tradeId: null } })];

export function bid(opts: Partial<Bid> & { id: string }): Bid {
  return {
    bidder: BOOK.reseller,
    price: MANA(8),
    createdAt: FIXTURE_NOW - DAY,
    updatedAt: FIXTURE_NOW - DAY,
    fingerprint: "",
    status: "open",
    seller: BOOK.buyer,
    network: "MATIC",
    chainId: CHAIN.POLYGON,
    contractAddress: BOOK.collection,
    expiresAt: FIXTURE_NOW + 20 * DAY,
    tokenId: "105312",
    tradeId: `bid-trade-${opts.id}`,
    tradeContractAddress: BOOK.marketplace.contract,
    ...opts,
  };
}

export const RECEIVED_BIDS: Bid[] = [bid({ id: "b1" }), bid({ id: "b2", price: MANA("9.75"), bidder: BOOK.creator })];
export const MADE_BIDS: Bid[] = [bid({ id: "b3", bidder: BOOK.buyer, seller: BOOK.reseller, tokenId: "77", price: MANA(3) })];
export const LEGACY_BID_ROW: Bid = bid({
  id: "b4",
  tradeId: undefined,
  tradeContractAddress: undefined,
  bidAddress: "0xb96697fa4a3361ba35b774a42c58daccaad1b8e1",
  blockchainId: `0x${"12".repeat(32)}`,
});

export function sale(opts: Partial<Sale> & { id: string }): Sale {
  return {
    itemId: "7",
    contractAddress: BOOK.collection,
    buyer: BOOK.reseller,
    chainId: CHAIN.POLYGON,
    network: "MATIC",
    price: MANA(3),
    seller: BOOK.creator,
    timestamp: FIXTURE_NOW - DAY,
    tokenId: "105400",
    txHash: `0x${"ab".repeat(32)}`,
    type: "mint",
    ...opts,
  };
}

export const SALES: Sale[] = [
  sale({ id: "s1" }),
  sale({ id: "s2", buyer: BOOK.buyer, timestamp: FIXTURE_NOW - 3 * DAY, price: MANA("2.3") }),
  sale({ id: "s3", type: "order", itemId: null, tokenId: "99", price: MANA(40), timestamp: FIXTURE_NOW - 10 * DAY }),
];

export const ACCOUNT: Account = {
  id: BOOK.creator,
  address: BOOK.creator,
  sales: 128,
  purchases: 4,
  spent: MANA(30),
  earned: MANA("1840.25"),
  royalties: MANA("12.5"),
  collections: 3,
};

export const COLLECTIONS: Collection[] = [
  {
    urn: `urn:decentraland:matic:collections-v2:${BOOK.collection}`,
    creator: BOOK.creator,
    name: "Neon Nights",
    contractAddress: BOOK.collection,
    createdAt: FIXTURE_NOW - 90 * DAY,
    updatedAt: FIXTURE_NOW - 10 * DAY,
    reviewedAt: FIXTURE_NOW - 80 * DAY,
    isOnSale: true,
    size: 12,
    network: "MATIC",
    chainId: CHAIN.POLYGON,
    firstListedAt: FIXTURE_NOW - 80 * DAY,
  },
];

export function list(opts: Partial<FavoriteList> & { id: string; name: string }): FavoriteList {
  return {
    userAddress: BOOK.buyer,
    createdAt: FIXTURE_NOW - 10 * DAY,
    updatedAt: FIXTURE_NOW - DAY,
    isPrivate: true,
    isDefaultList: false,
    itemsCount: 2,
    previewOfItemIds: [`${BOOK.collection}-7`, `${BOOK.collection}-8`],
    ...opts,
  };
}

export const LISTS: FavoriteList[] = [
  list({ id: DEFAULT_LIST_ID, name: "Favorites", isDefaultList: true, isPrivate: true, itemsCount: 3 }),
  list({ id: "list-private", name: "Winter fits", description: "Only for me", isPrivate: true }),
  list({ id: "list-shared", name: "Party pack", description: "Shared with friends", isPrivate: false, itemsCount: 1 }),
];

export const ITEM_RANKS: ItemRank[] = [
  { id: `${BOOK.collection}-7`, sales: 42, volume: MANA(420) },
  { id: `${BOOK.collection}-8`, sales: 30, volume: MANA("210.5") },
];
export const CREATOR_RANKS: CreatorRank[] = [{ id: BOOK.creator, sales: 128, earned: MANA("1840.25"), collections: 3, uniqueCollectors: 77 }];
export const COLLECTOR_RANKS: CollectorRank[] = [{ id: BOOK.buyer, purchases: 12, spent: MANA(64), uniqueAndMythicItems: 1, creatorsSupported: 5 }];
export const VOLUME: VolumeData = { sales: 1204, volume: MANA("98765.4321"), creatorsEarnings: MANA("2345.6"), daoEarnings: MANA("123.45") };
