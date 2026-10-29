// JSON <-> model for the serde forms in model/settlement.ts and model/purchase.ts. Decoders refuse
// (null) anything the Rust types would refuse to deserialize; they never fill a default.

import { parseEvmAddress } from "./address";
import { ASSET_KINDS, SALE_KINDS, parseAssetId, type AssetKind, type SaleKind } from "./asset";
import { isChainId } from "./chain";
import { manaToWire, parseManaWei, parseUsdCents } from "./money";
import type { PurchaseOffer } from "./purchase";
import {
  PAYMENT_METHODS,
  type Eligibility,
  type GroupTotal,
  type MissingFact,
  type PaymentMethod,
  type Quote,
  type Reason,
  type SettlementCall,
  type SettlementGroup,
  type SettlementGroupKey,
  type WalletGuidance,
} from "./settlement";

type Json = Record<string, unknown>;

const REASONS: readonly Reason[] = [
  "creditsFirst",
  "manaSecond",
  "signInForCredits",
  "secondarySale",
  "landOrRental",
  "notDecentralandItem",
  "bundle",
  "creditsChainUnsupported",
  "unsupportedCombination",
];
const MISSING: readonly MissingFact[] = ["contractRegistry", "collectionItem", "nameRegistry", "manaUsdRate"];

function obj(value: unknown): Json | null {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Json) : null;
}

function oneOf<T extends string>(list: readonly T[], value: unknown): T | null {
  return typeof value === "string" && (list as readonly string[]).includes(value) ? (value as T) : null;
}

function arrayOf<T>(value: unknown, decode: (v: unknown) => T | null): T[] | null {
  if (!Array.isArray(value)) return null;
  const out: T[] = [];
  for (const v of value) {
    const d = decode(v);
    if (d === null) return null;
    out.push(d);
  }
  return out;
}

export function decodePaymentMethod(value: unknown): PaymentMethod | null {
  return oneOf(PAYMENT_METHODS, value);
}

export function decodeReason(value: unknown): Reason | null {
  return oneOf(REASONS, value);
}

export function decodeAssetKind(value: unknown): AssetKind | null {
  return oneOf(ASSET_KINDS, value);
}

export function decodeSaleKind(value: unknown): SaleKind | null {
  return oneOf(SALE_KINDS, value);
}

export function decodeWalletGuidance(value: unknown): WalletGuidance | null {
  const v = obj(value);
  if (!v || !isChainId(v.chainId) || typeof v.nativeGasRequired !== "boolean") return null;
  if (v.account === null || v.account === undefined) {
    return { chainId: v.chainId, nativeGasRequired: v.nativeGasRequired, account: null };
  }
  const account = parseEvmAddress(v.account);
  return account ? { chainId: v.chainId, nativeGasRequired: v.nativeGasRequired, account } : null;
}

export function decodeEligibility(value: unknown): Eligibility | null {
  const v = obj(value);
  if (!v) return null;
  const allowed = arrayOf(v.allowed, decodePaymentMethod);
  const reasons = arrayOf(v.reasons, decodeReason);
  const def = v.default === null ? null : decodePaymentMethod(v.default);
  const guidance = v.guidance === null || v.guidance === undefined ? null : decodeWalletGuidance(v.guidance);
  if (!allowed || !reasons || (v.default !== null && def === null)) return null;
  if (v.guidance !== null && v.guidance !== undefined && guidance === null) return null;
  return { allowed, default: def, reasons, guidance };
}

export function decodeQuote(value: unknown): Quote | null {
  const v = obj(value);
  if (!v) return null;
  if (v.method === "credits") {
    const quoted = parseUsdCents(v.quoted);
    return quoted === null ? null : { method: "credits", quoted };
  }
  if (v.method === "mana") {
    const amount = parseManaWei(v.amount);
    return amount === null ? null : { method: "mana", amount };
  }
  return null;
}

export function encodeQuote(quote: Quote): Json {
  return quote.method === "credits"
    ? { method: "credits", quoted: quote.quoted }
    : { method: "mana", amount: manaToWire(quote.amount) };
}

export function decodeGroupTotal(value: unknown): GroupTotal | null {
  const v = obj(value);
  if (!v) return null;
  if (v.method === "credits") {
    const quoted = parseUsdCents(v.quoted);
    const charged = parseUsdCents(v.charged);
    return quoted === null || charged === null ? null : { method: "credits", quoted, charged };
  }
  if (v.method === "mana") {
    const amount = parseManaWei(v.amount);
    return amount === null ? null : { method: "mana", amount };
  }
  return null;
}

export function decodeSettlementCall(value: unknown): SettlementCall | null {
  const v = obj(value);
  if (!v) return null;
  if (v.kind === "accept" || v.kind === "acceptWithCoupon" || v.kind === "storeBuy") return { kind: v.kind };
  if (v.kind === "one") {
    const asset = parseAssetId(v.asset);
    return asset ? { kind: "one", asset } : null;
  }
  return null;
}

export function decodeSettlementGroupKey(value: unknown): SettlementGroupKey | null {
  const v = obj(value);
  if (!v) return null;
  const method = decodePaymentMethod(v.method);
  const contract = parseEvmAddress(v.contract);
  const call = decodeSettlementCall(v.call);
  if (!method || !isChainId(v.chainId) || !contract || !call) return null;
  return { method, chainId: v.chainId, contract, call };
}

export function encodeSettlementGroupKey(key: SettlementGroupKey): Json {
  return { ...key, call: { ...key.call } };
}

export function decodeSettlementGroup(value: unknown): SettlementGroup | null {
  const v = obj(value);
  if (!v) return null;
  const key = decodeSettlementGroupKey(v.key);
  const lines = arrayOf(v.lines, (n) => (typeof n === "number" && Number.isSafeInteger(n) && n >= 0 ? n : null));
  const quotes = arrayOf(v.quotes, decodeQuote);
  const total = decodeGroupTotal(v.total);
  if (!key || !lines || !quotes || !total || typeof v.manaRemainderAllowed !== "boolean") return null;
  if (lines.length !== quotes.length) return null;
  return { key, lines, quotes, total, manaRemainderAllowed: v.manaRemainderAllowed };
}

export function decodePurchaseOffer(value: unknown): PurchaseOffer | null {
  const v = obj(value);
  if (!v) return null;
  const asset = parseAssetId(v.asset);
  const kind = decodeAssetKind(v.kind);
  const sale = decodeSaleKind(v.sale);
  const eligibility = decodeEligibility(v.eligibility);
  const quotes = arrayOf(v.quotes, decodeQuote);
  const unavailable = arrayOf(v.unavailable ?? [], (u) => {
    const o = obj(u);
    const method = o ? decodePaymentMethod(o.method) : null;
    const missing = o ? oneOf(MISSING, o.missing) : null;
    return method && missing ? { method, missing } : null;
  });
  const settlement = arrayOf(v.settlement, (s) => {
    const o = obj(s);
    const method = o ? decodePaymentMethod(o.method) : null;
    const key = o ? decodeSettlementGroupKey(o.key) : null;
    return method && key && key.method === method ? { method, key } : null;
  });
  const available = v.available === null || v.available === undefined ? null : v.available;
  if (!asset || !kind || !sale || !eligibility || !quotes || !unavailable || !settlement) return null;
  if (typeof v.bundle !== "boolean") return null;
  if (available !== null && !(typeof available === "number" && Number.isSafeInteger(available) && available >= 0)) {
    return null;
  }
  const quotedAt = typeof v.quotedAt === "number" && Number.isFinite(v.quotedAt) ? v.quotedAt : 0;
  return { asset, kind, sale, bundle: v.bundle, eligibility, quotes, unavailable, settlement, available, quotedAt };
}

export function encodePurchaseOffer(offer: PurchaseOffer): Json {
  return {
    ...offer,
    quotes: offer.quotes.map(encodeQuote),
    settlement: offer.settlement.map((s) => ({ method: s.method, key: encodeSettlementGroupKey(s.key) })),
  };
}
