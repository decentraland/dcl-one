// Catalogue answers for the discovery and detail tests and stories. The rows describe the same assets
// payment/fixtures.ts builds server offers for (tests/shop/policy.rs facts: the live Polygon contracts
// of tests/fixtures/shop/chain-reads.json, item 7, token 42, the collection and reseller addresses),
// in the wire shapes of marketplace-server@ae872a4a5 src/ports/shop-catalog/types.ts, so a card, a
// detail page and the offer behind them agree. `catalogTransport(scenario)` answers the /shop-api
// catalogue routes through createShopClient's transport seam: `ok`, `empty`, `error` (500), `outage`
// (the 503 unavailable envelope) and `loading` (never answers).

import { createElement, useState, type ReactNode } from "react";
import { MemoryRouter, Route, Routes } from "react-router";

import type { AuthIdentity } from "../../data/auth/identity";
import { createShopClient, setShopClient, type ShopClient } from "../api/shop";
import { shopCart, type CartStore } from "../cart/store";
import type { EvmAddress } from "../model/address";
import { ShopOfferProvider, missingOfferRoute, type OfferSource } from "../detail/offers";
import { ShopWalletProvider } from "../wallet/context";
import type { ShopWalletHost } from "../wallet/session";
import type { ShopTransport, TransportRequest } from "../api/signer";
import { BOOK, ITEM, TOKEN } from "../payment/fixtures";
import type { UnifiedListingRaw } from "./api";
import { resetShopQueries } from "./query";
import type { RawCollectionItem } from "./lib/catalogItem";

export type CatalogScenario = "ok" | "empty" | "error" | "outage" | "loading";

export const COLLECTION = BOOK.collection;
export const CREATOR = BOOK.creator;
export const RESELLER = BOOK.reseller;
export const ITEM_ID = ITEM.toString();
export const TOKEN_ID = TOKEN.toString();

function thumb(hue: number, label: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128"><rect width="128" height="128" rx="24" fill="hsl(${hue} 70% 55%)"/><text x="64" y="76" font-size="28" text-anchor="middle" fill="white" font-family="sans-serif">${label}</text></svg>`;
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

const base = {
  source: "native" as const,
  acquisition: "trade" as const,
  seller: null,
  issuedId: null,
  compareAtCredits: null,
  saleEndsAt: null,
  saleUnitsLeft: null,
  coupon: null,
  network: "MATIC",
  chainId: 137,
  gender: "unisex" as const,
  creator: CREATOR,
  contractAddress: COLLECTION,
  tokenId: null,
  manaWei: null,
};

/** A creator's USD-pegged listing of item 7 ($1.15 = 12 credits): eligible primary. */
export const PRIMARY_WEARABLE: UnifiedListingRaw & { listingCount: number } = {
  ...base,
  tradeId: "trade-primary-7",
  listingType: "primary",
  itemId: ITEM_ID,
  name: "Nacho Hat",
  thumbnail: thumb(280, "Hat"),
  rarity: "epic",
  category: "wearable",
  wearableCategory: "hat",
  priceCredits: 12,
  available: 10,
  listingCount: 2,
};

/** A CollectionStore mint of an emote, MANA-priced (4.02 MANA, $2.01 = 21 credits at the $0.50 rate). */
export const STORE_EMOTE: UnifiedListingRaw & { listingCount: number } = {
  ...base,
  source: "legacy",
  acquisition: "store",
  tradeId: null,
  listingType: "primary",
  itemId: "8",
  name: "Hot Sax",
  thumbnail: thumb(20, "Sax"),
  rarity: "legendary",
  category: "emote",
  wearableCategory: "dance",
  emoteLoop: true,
  gender: null,
  priceCredits: 21,
  manaWei: "4020000000000000000",
  available: 48,
  listingCount: 1,
};

/** A running creator sale: 14 credits instead of 20, ending in a day. */
export function saleWearable(now: number = Date.now()): UnifiedListingRaw & { listingCount: number } {
  return {
    ...base,
    tradeId: "trade-sale-9",
    listingType: "primary",
    itemId: "9",
    name: "Flash Jacket",
    thumbnail: thumb(200, "Sale"),
    rarity: "rare",
    category: "wearable",
    wearableCategory: "upper_body",
    priceCredits: 14,
    compareAtCredits: 20,
    saleEndsAt: Math.floor(now / 1000) + 86_400,
    saleUnitsLeft: 3,
    available: 3,
    listingCount: 1,
  };
}

/** Token 42 of item 7, resold by a reseller: a secondary sale, MANA only, whatever `priceCredits` says. */
export const RESALE_WEARABLE: UnifiedListingRaw & { listingCount: number } = {
  ...base,
  tradeId: "trade-resale-42",
  listingType: "secondary",
  itemId: ITEM_ID,
  tokenId: TOKEN_ID,
  name: "Nacho Hat",
  thumbnail: thumb(280, "#42"),
  rarity: "epic",
  category: "wearable",
  wearableCategory: "hat",
  seller: RESELLER,
  issuedId: "42",
  priceCredits: 10,
  available: 1,
  listingCount: 1,
};

/** An Ethereum (L1) mint: MANA only, credits settle on Polygon. */
export const L1_MINT: UnifiedListingRaw & { listingCount: number } = {
  ...base,
  source: "legacy",
  acquisition: "store",
  tradeId: null,
  listingType: "primary",
  itemId: "3",
  name: "Genesis Boots",
  thumbnail: thumb(120, "L1"),
  rarity: "mythic",
  category: "wearable",
  wearableCategory: "feet",
  network: "ETHEREUM",
  chainId: 1,
  priceCredits: 60,
  manaWei: "12000000000000000000",
  available: 4,
  listingCount: 1,
};

/** An eligible listing whose credit price the feed did not give: an outage, not MANA. */
export const UNPRICED_WEARABLE: UnifiedListingRaw & { listingCount: number } = {
  ...PRIMARY_WEARABLE,
  tradeId: "trade-unpriced-11",
  itemId: "11",
  name: "Unpriced Cape",
  thumbnail: thumb(330, "?"),
  priceCredits: 0,
  listingCount: 1,
};

export const UNIFIED_ROWS = [PRIMARY_WEARABLE, STORE_EMOTE, RESALE_WEARABLE, L1_MINT, UNPRICED_WEARABLE];

export function catalogRow(row: UnifiedListingRaw, onSale = true): RawCollectionItem {
  return {
    id: `${row.contractAddress}-${row.itemId}`,
    name: row.name,
    creator: row.creator,
    contractAddress: row.contractAddress,
    itemId: row.itemId,
    category: row.category,
    rarity: row.rarity,
    network: row.network,
    chainId: row.chainId,
    thumbnail: row.thumbnail,
    priceCredits: onSale ? row.priceCredits : 0,
    // A trade row carries its USD price in wei (one credit is $0.10); a store mint its MANA price.
    price: onSale ? (row.manaWei ?? (row.tradeId && row.priceCredits > 0 ? String(BigInt(row.priceCredits) * 10n ** 17n) : null)) : null,
    tradeId: onSale ? row.tradeId : null,
    available: row.available,
    isOnSale: onSale,
    data: row.category === "emote" ? { emote: { category: row.wearableCategory ?? undefined, loop: true } } : { wearable: { category: row.wearableCategory ?? undefined, bodyShapes: ["BaseMale", "BaseFemale"] } },
  };
}

export const COLLECTION_META = { contractAddress: COLLECTION, name: "Nacho Collection", creator: CREATOR, size: 3, isOnSale: true, chainId: 137, network: "MATIC" };

export const ITEM_META = {
  name: "Nacho Hat",
  thumbnail: PRIMARY_WEARABLE.thumbnail,
  utility: "Glows at night",
  urn: `urn:decentraland:matic:collections-v2:${COLLECTION}:${ITEM_ID}`,
  available: "10",
  creator: CREATOR,
  rarity: "epic",
  category: "wearable",
  chainId: 137,
  network: "MATIC",
  data: { wearable: { isSmart: false, category: "hat", bodyShapes: ["BaseMale", "BaseFemale"] } },
};

export function tokenNft(owner: string = RESELLER, onSale = true) {
  return {
    nft: {
      id: `${COLLECTION}-${TOKEN_ID}`,
      activeOrderId: null,
      category: "wearable",
      chainId: 137,
      contractAddress: COLLECTION,
      createdAt: 0,
      data: { wearable: { rarity: "epic", category: "hat" } },
      image: RESALE_WEARABLE.thumbnail,
      issuedId: "42",
      itemId: ITEM_ID,
      name: "Nacho Hat",
      network: "MATIC",
      openRentalId: null,
      owner,
      tokenId: TOKEN_ID,
      soldAt: 0,
      updatedAt: 0,
      url: "",
    },
    order: onSale ? { price: "2000000000000000000", tradeId: RESALE_WEARABLE.tradeId, owner } : null,
    rental: null,
  };
}

export const SUGGESTIONS = {
  items: { data: [{ ...catalogRow(PRIMARY_WEARABLE), creatorName: "Nacho" }], total: 1 },
  collections: { data: [{ contractAddress: COLLECTION, name: "Nacho Collection", creator: CREATOR, creatorName: "Nacho", items: 3, sales: 9 }] },
  creators: { data: [{ address: CREATOR, name: "Nacho", face: null }] },
};

const OUTAGE_BODY = JSON.stringify({ error: "Service Unavailable", message: "the items mirror has no complete pass yet", source: "itemsMirror", configure: "MARKETPLACE_MIRRORS_ENABLED" });

export type CatalogAnswers = Partial<Record<string, unknown | ((url: URL) => unknown)>>;

/** The default answer per canonical path (without the /shop-api prefix). */
export function defaultAnswers(): Record<string, (url: URL) => unknown> {
  return {
    "/v3/catalog/unified": (url) => {
      const itemId = url.searchParams.get("itemId");
      const listingType = url.searchParams.get("listingType");
      const discounted = url.searchParams.get("discounted");
      let rows: UnifiedListingRaw[] = discounted === "true" ? [saleWearable()] : UNIFIED_ROWS;
      if (itemId) rows = [PRIMARY_WEARABLE, RESALE_WEARABLE, STORE_EMOTE, L1_MINT, UNPRICED_WEARABLE, saleWearable()].filter((r) => r.itemId === itemId);
      if (listingType) rows = rows.filter((r) => r.listingType === listingType);
      return { data: rows, total: rows.length };
    },
    "/v3/catalog/shop": () => ({ data: [PRIMARY_WEARABLE], total: 1 }),
    "/v3/catalog/items": (url) => {
      const onSale = url.searchParams.get("isOnSale");
      const rows = [PRIMARY_WEARABLE, STORE_EMOTE, L1_MINT].map((r) => catalogRow(r, onSale !== "false"));
      return { data: rows, total: rows.length };
    },
    "/v3/catalog/trending": () => ({ data: [PRIMARY_WEARABLE, STORE_EMOTE] }),
    "/v3/catalog/related": () => ({ data: [STORE_EMOTE] }),
    "/v3/catalog/suggested": () => ({ data: [], personalized: false, algorithm: "fixture" }),
    "/v3/catalog/suggest": () => SUGGESTIONS,
    "/v3/catalog/creators": () => ({ data: [] }),
    "/v1/collections": () => ({ data: [COLLECTION_META], total: 1 }),
    "/v1/items": (url) => {
      const itemId = url.searchParams.get("itemId");
      const known = UNIFIED_ROWS.find((r) => r.itemId === itemId);
      if (itemId && !known) return { data: [], total: 0 };
      return { data: [known && known !== PRIMARY_WEARABLE && known !== RESALE_WEARABLE ? { ...ITEM_META, name: known.name, thumbnail: known.thumbnail, rarity: known.rarity, category: known.category } : ITEM_META], total: 1 };
    },
    "/v2/catalog": () => ({ data: [{ ...catalogRow(PRIMARY_WEARABLE), data: { wearable: { description: "A hat for the discerning nacho." } } }], total: 1 }),
    "/v1/nfts": (url) => {
      const owner = url.searchParams.get("owner");
      if (url.searchParams.get("tokenId")) {
        if (owner && owner !== RESELLER) return { data: [], total: 0 };
        return { data: [tokenNft()], total: 1 };
      }
      return { data: [], total: 0 };
    },
    "/v1/orders": () => ({ data: [], total: 0 }),
    "/v1/trades-mirror": () => ({ ok: true, data: { enabled: true, completedAt: new Date(Date.now() - 5 * 60_000).toISOString(), inProgress: false, pass: 12 } }),
  };
}

/** A transport answering the catalogue routes; `overrides` replaces or adds answers per path. */
export function catalogTransport(scenario: CatalogScenario = "ok", overrides: CatalogAnswers = {}): ShopTransport & { calls: TransportRequest[] } {
  const calls: TransportRequest[] = [];
  const answers = { ...defaultAnswers(), ...overrides };
  const transport = async (req: TransportRequest) => {
    calls.push(req);
    const url = new URL(req.url, "http://shop.test");
    const path = url.pathname.replace(/^\/shop-api/, "").replace(/^\/marketplace-api/, "");
    if (scenario === "loading") return new Promise<never>(() => {});
    if (scenario === "error") return { status: 500, body: JSON.stringify({ error: "boom" }) };
    if (scenario === "outage") return { status: 503, body: OUTAGE_BODY };
    const answer = answers[path];
    if (answer === undefined) return { status: 404, body: JSON.stringify({ error: `no fixture for ${path}` }) };
    if (scenario === "empty") return { status: 200, body: JSON.stringify(path.endsWith("/suggested") ? { data: [], personalized: false } : { data: [], total: 0 }) };
    const body = typeof answer === "function" ? (answer as (u: URL) => unknown)(url) : answer;
    return { status: 200, body: JSON.stringify(body) };
  };
  return Object.assign(transport, { calls });
}

export function catalogClient(scenario: CatalogScenario = "ok", overrides: CatalogAnswers = {}): ShopClient & { calls: TransportRequest[] } {
  const transport = catalogTransport(scenario, overrides);
  return Object.assign(createShopClient({ base: "http://shop.test", mode: "canonical", transport, signer: null }), { calls: transport.calls });
}

export type SessionKind = "signedOut" | "web2" | "web3";

export function fixtureIdentity(signer: string = BOOK.buyer, ms = 3_600_000): AuthIdentity {
  return {
    signer,
    ephemeral: { address: "0x0000000000000000000000000000000000000001", privateKey: `0x${"11".repeat(32)}` },
    expiration: new Date(Date.now() + ms).toISOString(),
    authChain: [],
  };
}

/** A host for ShopWalletProvider: signed out, a web2 (managed) account, or a web3 wallet. */
export function fixtureHost(kind: SessionKind, account: EvmAddress = BOOK.buyer, actions: Partial<ShopWalletHost> = {}): ShopWalletHost {
  if (kind === "signedOut") return { identity: null, walletKind: null, ...actions };
  return { identity: fixtureIdentity(account), walletKind: kind === "web3" ? "web3" : "managed", ...actions };
}

export type HarnessProps = {
  children: ReactNode;
  /** The route pattern the page is mounted at (e.g. `/shop/item/:contractAddress/:itemId`). */
  path?: string;
  /** Where the memory router starts. */
  entry?: string;
  session?: SessionKind;
  scenario?: CatalogScenario;
  overrides?: CatalogAnswers;
  client?: ShopClient;
  offers?: OfferSource;
  host?: ShopWalletHost;
  cart?: CartStore;
};

/**
 * Mounts a Shop page the way a host does: its route inside a memory router, the wallet seam, the offer
 * seam and the document's Shop client (installed before the first render). Stories and tests share it.
 */
export function ShopHarness({ children, path = "*", entry = "/", session = "signedOut", scenario = "ok", overrides, client, offers = missingOfferRoute, host, cart }: HarnessProps) {
  const [installed] = useState(() => {
    resetShopQueries();
    setShopClient(client ?? catalogClient(scenario, overrides));
    return true;
  });
  const [store] = useState(() => cart ?? shopCart);
  const [walletHost] = useState(() => host ?? fixtureHost(session));
  if (!installed) return null;
  const routes = createElement(Routes, null, createElement(Route, { path, element: children }));
  const withOffers = createElement(ShopOfferProvider, { source: offers, children: routes });
  const withWallet = createElement(ShopWalletProvider, { host: walletHost, cart: store, children: withOffers });
  return createElement(MemoryRouter, { initialEntries: [entry] }, withWallet);
}
