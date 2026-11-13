// Mirrors the payment and settlement types of src/shop/types.rs and src/shop/policy.rs, in the serde
// forms those modules declare (`rename_all = "camelCase"`, internally tagged enums). MANA amounts are
// bigints here and decimal strings on the wire; model/wire.ts converts.

import type { EvmAddress } from "./address";
import { assetKey, type AssetId, type AssetKind, type ContractRef, type NameLabel } from "./asset";
import type { ChainId } from "./chain";
import type { ManaWei, UsdCents } from "./money";

export type PaymentMethod = "credits" | "mana";
/** In the order `PaymentMethod` sorts in Rust: credits before MANA. */
export const PAYMENT_METHODS: readonly PaymentMethod[] = ["credits", "mana"];

/** Upstream `MAX_LINES_PER_GROUP` (credits-server `authorize-usd-credit-batch.ts`). */
export const MAX_CREDIT_LINES_PER_GROUP = 30;

/** The call a settlement group makes on its contract. serde: `{"kind":"accept"}`, `{"kind":"one","asset":…}`. */
export type SettlementCall =
  | { kind: "accept" }
  | { kind: "acceptWithCoupon" }
  | { kind: "storeBuy" }
  | { kind: "one"; asset: AssetId };

/** One confirmation group: exactly what one transaction settles. */
export type SettlementGroupKey = {
  method: PaymentMethod;
  chainId: ChainId;
  contract: EvmAddress;
  call: SettlementCall;
};

/** Who is buying, from the authenticated session; never inferred from an address or a balance. */
export type Session =
  | { kind: "signedOut" }
  | { kind: "web2"; account: EvmAddress }
  | { kind: "web3"; wallet: EvmAddress };

export const SIGNED_OUT: Session = { kind: "signedOut" };

export function hasWallet(session: Session): boolean {
  return session.kind === "web3";
}

export function sessionAccount(session: Session): EvmAddress | null {
  if (session.kind === "web2") return session.account;
  if (session.kind === "web3") return session.wallet;
  return null;
}

export type Reason =
  | "creditsFirst"
  | "manaSecond"
  | "signInForCredits"
  | "secondarySale"
  | "landOrRental"
  | "notDecentralandItem"
  | "bundle"
  | "creditsChainUnsupported"
  | "unsupportedCombination";

/** The reasons that present credits; a MANA-only line never carries one. */
export const CREDIT_REASONS: readonly Reason[] = ["creditsFirst", "manaSecond", "signInForCredits"];

/** For a session without a web3 wallet on a MANA-only purchase. It never suggests credits. */
export type WalletGuidance = {
  chainId: ChainId;
  nativeGasRequired: boolean;
  /** The signed-in account whose wallet to set up; null when signed out. */
  account: EvmAddress | null;
};

/** `policy::Eligibility`: which methods a line offers, in presentation order. */
export type Eligibility = {
  allowed: PaymentMethod[];
  default: PaymentMethod | null;
  reasons: Reason[];
  guidance: WalletGuidance | null;
};

export type MissingFact = "contractRegistry" | "collectionItem" | "nameRegistry" | "manaUsdRate";

/** `policy::Quote`: what one line costs in its chosen method. */
export type Quote = { method: "credits"; quoted: UsdCents } | { method: "mana"; amount: ManaWei };

/** `policy::GroupTotal`: `charged` sums each line's whole-credit charge. */
export type GroupTotal = { method: "credits"; quoted: UsdCents; charged: UsdCents } | { method: "mana"; amount: ManaWei };

/** `policy::SettlementGroup`, as the server partitions a cart. */
export type SettlementGroup = {
  key: SettlementGroupKey;
  /** Indexes into the cart, in cart order. */
  lines: number[];
  quotes: Quote[];
  total: GroupTotal;
  manaRemainderAllowed: boolean;
};

/** A stable string for a settlement key (maps, React keys, persisted progress). */
export function settlementKeyId(key: SettlementGroupKey): string {
  const call = key.call.kind === "one" ? `one:${assetKey(key.call.asset)}` : key.call.kind;
  return `${key.method}|${key.chainId}|${key.contract}|${call}`;
}

export function sameSettlementKey(a: SettlementGroupKey, b: SettlementGroupKey): boolean {
  return settlementKeyId(a) === settlementKeyId(b);
}

/** A creator discount applied at purchase through `acceptWithCoupon` (`types::TradeCoupon`). */
export type TradeCoupon = { contract: EvmAddress; discountPpm: number };

export type TradeAssetType = "erc20" | "usdPeggedMana" | "erc721" | "collectionItem" | { unsupported: number };

export type TradeAsset = { assetType: TradeAssetType; contract: EvmAddress; value: string };

export type SignedTrade = { venue: ContractRef; signer: EvmAddress; sent: TradeAsset[]; received: TradeAsset[] };

/**
 * `types::Offer`: what a line offers to sell, from an authoritative source. Rust does not serialize it
 * yet; this mirrors its variants with the module's serde conventions for when a route does. The UI
 * never builds one to decide eligibility: it renders the server's `PurchaseOffer` (model/purchase.ts).
 */
export type Offer =
  | { kind: "storeMint"; venue: ContractRef; collection: EvmAddress; itemId: string; price: ManaWei }
  | { kind: "trade"; trade: SignedTrade; coupon: TradeCoupon | null }
  | { kind: "legacyOrder"; venue: ContractRef; nft: EvmAddress; tokenId: string; price: ManaWei }
  | {
      kind: "nameRegistration";
      label: NameLabel;
      registrar: ContractRef;
      manaVenue: ContractRef;
      creditsVenue: ContractRef;
      price: ManaWei;
    }
  | { kind: "rental"; venue: ContractRef; contract: EvmAddress; tokenId: string; price: ManaWei };

/** The asset kinds no credit ever pays for. */
export function isLandKind(kind: AssetKind): boolean {
  return kind === "parcel" || kind === "estate" || kind === "rental";
}
