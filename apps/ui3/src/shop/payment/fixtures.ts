// Server answers for tests and stories, built like tests/shop/policy.rs builds its facts: the live
// Polygon contracts from tests/fixtures/shop/chain-reads.json, synthetic addresses for the rest, the
// same item/token ids and prices, and the rate $0.50 per MANA (upstream mana-rate.spec RATE_HALF).
// Each offer carries what the server's rule answers for it (rule.ts mirrors policy.rs), so tests of
// the rendering start from a consistent server and then break it on purpose.

import { evmAddress, type EvmAddress } from "../model/address";
import {
  itemAsset,
  nameAsset,
  parseNameLabel,
  tokenAsset,
  type AssetId,
  type AssetKind,
  type ContractRef,
  type SaleKind,
} from "../model/asset";
import { CHAIN, type ChainId } from "../model/chain";
import {
  centsFromUsdWeiCeil,
  discountedCents,
  discountedMana,
  manaFromUsdWei,
  manaFromWhole,
  manaUsdCentsCeil,
  manaUsdRate,
  manaWei,
  type ManaWei,
  type UsdCents,
} from "../model/money";
import type { PurchaseOffer } from "../model/purchase";
import type { PaymentMethod, Quote, Session, SettlementCall, SettlementGroupKey } from "../model/settlement";
import { present, restrictions } from "./rule";

function synthetic(n: number): EvmAddress {
  return evmAddress(`0x${n.toString(16).padStart(40, "0")}`);
}

const at = (chainId: ChainId, contract: EvmAddress): ContractRef => ({ chainId, contract });

export const BOOK = {
  mana: evmAddress("0xa1c57f48f0deb89f569dfbe6e2b7f46d33606fd4"),
  marketplace: at(CHAIN.POLYGON, evmAddress("0xe38ef22abe871513555cba89adfe45ab4f548ada")),
  store: at(CHAIN.POLYGON, evmAddress("0x214ffc0f0103735728dc66b61a22e4f163e275ae")),
  legacy: at(CHAIN.POLYGON, evmAddress("0x480a0f4e360e8964e68858dd231c2922f1df45ef")),
  executor: at(CHAIN.POLYGON, evmAddress("0xc9a9c416fbfd424d09e3f905142799abb10c2fa7")),
  registrar: at(CHAIN.ETHEREUM, evmAddress("0x2a187453064356c898cae034eaed119e1663acb8")),
  controller: at(CHAIN.ETHEREUM, synthetic(0xc0)),
  ethMarketplace: at(CHAIN.ETHEREUM, synthetic(0xe1)),
  ethLegacy: at(CHAIN.ETHEREUM, synthetic(0xe2)),
  collection: synthetic(0xc011),
  land: synthetic(0x1a4d),
  estate: synthetic(0xe57a),
  rentals: at(CHAIN.ETHEREUM, synthetic(0x4e47)),
  creator: synthetic(0xc4ea),
  buyer: synthetic(0xb0b),
  reseller: synthetic(0x5e11),
} as const;

export const ITEM = 7n;
export const TOKEN = 42n;

/** $0.50 per MANA, 8 decimals. */
export const RATE = manaUsdRate(50_000_000n, 8)!;

export const SIGNED_OUT: Session = { kind: "signedOut" };
export const web2 = (account: EvmAddress = BOOK.buyer): Session => ({ kind: "web2", account });
export const web3 = (wallet: EvmAddress = BOOK.buyer): Session => ({ kind: "web3", wallet });
export const SESSIONS: readonly Session[] = [SIGNED_OUT, web2(), web3()];

/** The price an offer's source sets: USD wei for a USD-pegged trade, MANA wei otherwise. */
type Price = { usdWei: bigint } | { manaWei: ManaWei };

type Venues = Partial<Record<PaymentMethod, ContractRef>>;

type Spec = {
  asset: AssetId;
  kind: AssetKind;
  sale: SaleKind;
  bundle?: boolean;
  price: Price;
  couponPpm?: number;
  venues: Venues;
  call: SettlementCall;
  available?: number | null;
};

function creditsQuote(spec: Spec): UsdCents | null {
  const cents =
    "usdWei" in spec.price ? centsFromUsdWeiCeil(spec.price.usdWei) : manaUsdCentsCeil(spec.price.manaWei, RATE);
  if (cents === null) return null;
  return spec.couponPpm ? discountedCents(cents, spec.couponPpm) : cents;
}

function manaQuote(spec: Spec): ManaWei | null {
  const wei = "usdWei" in spec.price ? manaFromUsdWei(spec.price.usdWei, RATE) : spec.price.manaWei;
  if (wei === null) return null;
  return spec.couponPpm ? discountedMana(wei, spec.couponPpm) : wei;
}

/** What the server answers for `spec` and `session`: policy::eligibility, quote and settlement_key. */
export function serverOffer(spec: Spec, session: Session): PurchaseOffer {
  const bundle = spec.bundle ?? false;
  const manaChain = spec.venues.mana?.chainId ?? spec.asset.chainId;
  const eligibility = present(restrictions(spec.kind, spec.sale, spec.asset.chainId, bundle), session, manaChain);
  const quotes: Quote[] = [];
  const settlement: { method: PaymentMethod; key: SettlementGroupKey }[] = [];
  for (const method of eligibility.allowed) {
    const venue = spec.venues[method];
    if (!venue) continue;
    if (method === "credits") {
      const quoted = creditsQuote(spec);
      if (quoted !== null) quotes.push({ method, quoted });
    } else {
      const amount = manaQuote(spec);
      if (amount !== null) quotes.push({ method, amount });
    }
    settlement.push({ method, key: { method, chainId: venue.chainId, contract: venue.contract, call: spec.call } });
  }
  return {
    asset: spec.asset,
    kind: spec.kind,
    sale: spec.sale,
    bundle,
    eligibility,
    quotes,
    unavailable: [],
    settlement,
    available: spec.available ?? null,
    quotedAt: 0,
  };
}

const ONE_USD = { usdWei: 10n ** 18n };

/** 2.3 MANA for the wearable ($1.15 at RATE), 4.02 MANA for the emote ($2.01). */
export function storePrice(kind: "wearable" | "emote"): ManaWei {
  return manaWei(BigInt(kind === "emote" ? 4_020 : 2_300) * 10n ** 15n);
}

export function storeMint(session: Session, kind: "wearable" | "emote" = "wearable", itemId = ITEM): PurchaseOffer {
  return serverOffer(
    {
      asset: itemAsset(CHAIN.POLYGON, BOOK.collection, itemId),
      kind,
      sale: "primaryMint",
      price: { manaWei: storePrice(kind) },
      venues: { credits: BOOK.store, mana: BOOK.store },
      call: { kind: "storeBuy" },
      available: 10,
    },
    session,
  );
}

export function creatorOrder(session: Session, kind: "wearable" | "emote" = "wearable", couponPpm?: number): PurchaseOffer {
  return serverOffer(
    {
      asset: itemAsset(CHAIN.POLYGON, BOOK.collection, ITEM),
      kind,
      sale: "primaryMint",
      price: ONE_USD,
      couponPpm,
      venues: { credits: BOOK.marketplace, mana: BOOK.marketplace },
      call: { kind: couponPpm ? "acceptWithCoupon" : "accept" },
    },
    session,
  );
}

/** A minted token resold on the Polygon marketplace, by a reseller or by its own creator. */
export function wearableResale(session: Session, kind: "wearable" | "emote" = "wearable"): PurchaseOffer {
  return serverOffer(
    {
      asset: tokenAsset(CHAIN.POLYGON, BOOK.collection, TOKEN),
      kind,
      sale: "secondary",
      price: ONE_USD,
      venues: { credits: BOOK.marketplace, mana: BOOK.marketplace },
      call: { kind: "accept" },
    },
    session,
  );
}

export function nameRegistration(session: Session, label = "Nacho"): PurchaseOffer {
  const asset = nameAsset(BOOK.registrar, parseNameLabel(label)!);
  return serverOffer(
    {
      asset,
      kind: "name",
      sale: "primaryNameRegistration",
      price: { manaWei: manaFromWhole(100) },
      venues: { credits: BOOK.executor, mana: BOOK.controller },
      call: { kind: "one", asset },
    },
    session,
  );
}

export function nameResale(session: Session): PurchaseOffer {
  return serverOffer(
    {
      asset: tokenAsset(CHAIN.ETHEREUM, BOOK.registrar.contract, TOKEN),
      kind: "name",
      sale: "secondary",
      price: ONE_USD,
      venues: { mana: BOOK.ethMarketplace },
      call: { kind: "accept" },
    },
    session,
  );
}

export function parcelResale(session: Session): PurchaseOffer {
  const asset = tokenAsset(CHAIN.ETHEREUM, BOOK.land, TOKEN);
  return serverOffer(
    {
      asset,
      kind: "parcel",
      sale: "secondary",
      price: { manaWei: manaFromWhole(4000) },
      venues: { mana: BOOK.ethLegacy },
      call: { kind: "one", asset },
    },
    session,
  );
}

export function estateResale(session: Session): PurchaseOffer {
  return serverOffer(
    {
      asset: tokenAsset(CHAIN.ETHEREUM, BOOK.estate, TOKEN),
      kind: "estate",
      sale: "secondary",
      price: ONE_USD,
      venues: { mana: BOOK.ethMarketplace },
      call: { kind: "accept" },
    },
    session,
  );
}

export function rental(session: Session): PurchaseOffer {
  const asset = tokenAsset(CHAIN.ETHEREUM, BOOK.land, TOKEN);
  return serverOffer(
    {
      asset,
      kind: "rental",
      sale: "secondary",
      price: { manaWei: manaFromWhole(30) },
      venues: { mana: BOOK.rentals },
      call: { kind: "one", asset },
    },
    session,
  );
}

export function foreignMint(session: Session): PurchaseOffer {
  return serverOffer(
    {
      asset: itemAsset(CHAIN.POLYGON, synthetic(0xf0), ITEM),
      kind: "other",
      sale: "primaryMint",
      price: { manaWei: manaFromWhole(12) },
      venues: { mana: BOOK.store },
      call: { kind: "storeBuy" },
    },
    session,
  );
}

export function foreignResale(session: Session): PurchaseOffer {
  return serverOffer(
    {
      asset: tokenAsset(CHAIN.POLYGON, synthetic(0xf0), TOKEN),
      kind: "other",
      sale: "secondary",
      price: ONE_USD,
      venues: { mana: BOOK.marketplace },
      call: { kind: "accept" },
    },
    session,
  );
}

export function l1Mint(session: Session): PurchaseOffer {
  const venue = at(CHAIN.ETHEREUM, synthetic(0x5701));
  return serverOffer(
    {
      asset: itemAsset(CHAIN.ETHEREUM, BOOK.collection, ITEM),
      kind: "wearable",
      sale: "primaryMint",
      price: { manaWei: manaFromWhole(12) },
      venues: { mana: venue },
      call: { kind: "storeBuy" },
    },
    session,
  );
}

export function bundleOrder(session: Session): PurchaseOffer {
  return serverOffer(
    {
      asset: itemAsset(CHAIN.POLYGON, BOOK.collection, ITEM),
      kind: "wearable",
      sale: "primaryMint",
      bundle: true,
      price: ONE_USD,
      venues: { mana: BOOK.marketplace },
      call: { kind: "accept" },
    },
    session,
  );
}

/** The eleven AssetKind x SaleKind pairs an offer can express (tests/shop/policy.rs `facts_for`). */
export function offerFor(kind: AssetKind, sale: SaleKind, session: Session): PurchaseOffer | null {
  if ((kind === "wearable" || kind === "emote") && sale === "primaryMint") return storeMint(session, kind);
  if ((kind === "wearable" || kind === "emote") && sale === "secondary") return wearableResale(session, kind);
  if (kind === "name" && sale === "primaryNameRegistration") return nameRegistration(session);
  if (kind === "name" && sale === "secondary") return nameResale(session);
  if (kind === "parcel" && sale === "secondary") return parcelResale(session);
  if (kind === "estate" && sale === "secondary") return estateResale(session);
  if (kind === "rental" && sale === "secondary") return rental(session);
  if (kind === "other" && sale === "primaryMint") return foreignMint(session);
  if (kind === "other" && sale === "secondary") return foreignResale(session);
  return null;
}
