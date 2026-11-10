// from: shop@94584a5c:app/src/lib/currency.ts, app/src/lib/mana-convert.ts
// The credit unit under upstream's names, built on model/money (the exact helpers that mirror
// src/shop/types.rs): one credit is ten US cents; a price or a charge rounds UP to whole credits, a
// balance DOWN. Conversions are exact integer/bigint arithmetic. Every function here is presentation:
// a displayed conversion is never authority to change a charge (SHOP.md), and a line the payment rule
// makes MANA-only never gets a credit price from here (`displayCredits` answers null for it).

import {
  CENTS_PER_CREDIT,
  creditsCeil,
  creditsFloor,
  manaUsdCentsCeil,
  parseManaWei,
  type ManaUsdRate,
  type ManaWei,
  type UsdCents,
} from "../../model/money";

export const CURRENCY = { name: "Credits", nameSingular: "Credit" } as const;

/** THE PEG: one credit is ten US cents (upstream `USD_CENTS_PER_CREDIT`). */
export const USD_CENTS_PER_CREDIT = CENTS_PER_CREDIT;

/** Credits to US dollars. Exact: a credit is a whole number of cents. */
export function creditsToUsd(credits: number): number {
  return (credits * USD_CENTS_PER_CREDIT) / 100;
}

function wholeCents(cents: number): UsdCents | null {
  return Number.isSafeInteger(cents) ? (cents as UsdCents) : null;
}

/** Cents to credits, rounded UP: for anything the buyer must cover (a price, a charge, a top-up). */
export function usdCentsToCredits(cents: number): number {
  const whole = wholeCents(cents);
  return whole === null ? Math.ceil(cents / USD_CENTS_PER_CREDIT) : creditsCeil(whole);
}

/** Cents to credits, rounded DOWN: for anything the buyer holds (a balance): never a credit that is not there. */
export function usdCentsToCreditsFloor(cents: number): number {
  const whole = wholeCents(cents);
  return whole === null ? Math.floor(cents / USD_CENTS_PER_CREDIT) : creditsFloor(whole);
}

/** The unit word for an amount: "1 Credit", "2 Credits". */
export function creditsUnit(n: number): string {
  return n === 1 ? CURRENCY.nameSingular : CURRENCY.name;
}

function isOneCredit(value: unknown): boolean {
  if (typeof value === "number") return value === 1;
  if (typeof value === "string") return Number(value.trim().replace(/,/g, "")) === 1;
  return false;
}

/** The unit word for a displayed amount that may be a formatted string ("1", "1,250"): "credit"/"credits". */
export function creditsNoun(value: unknown, capitalized = false): string {
  const noun = isOneCredit(value) ? CURRENCY.nameSingular : CURRENCY.name;
  return capitalized ? noun : noun.toLowerCase();
}

/** The screen-reader suffix after a bare number printed next to the credit mark: " credits". */
export function creditsSrLabel(value: unknown): string {
  return " " + creditsNoun(value);
}

/** "270 Credits" / "1 Credit". */
export function formatAmount(n: number): string {
  return `${n} ${creditsUnit(n)}`;
}

const compactCache = new Map<string, Intl.NumberFormat>();
const fullCache = new Map<string, Intl.NumberFormat>();

function formatterFor(cache: Map<string, Intl.NumberFormat>, locale: string, opts: Intl.NumberFormatOptions): Intl.NumberFormat {
  let f = cache.get(locale);
  if (!f) {
    f = new Intl.NumberFormat(locale, opts);
    cache.set(locale, f);
  }
  return f;
}

/** Compact, for tight spaces: 12_000 -> "12K". Lossy: pair it with a title carrying `formatCreditsFull`. */
export function formatCredits(n: number, locale = "en"): string {
  return formatterFor(compactCache, locale, { notation: "compact", maximumFractionDigits: 2 }).format(n);
}

/** Full and grouped: 5_500_000 -> "5,500,000". */
export function formatCreditsFull(n: number, locale = "en"): string {
  return formatterFor(fullCache, locale, {}).format(n);
}

// ---- mana-convert: MANA <-> USD at a given rate, pure bigint arithmetic ----

/** Upstream `ManaRate`: the aggregator answer and its decimals (model/money `ManaUsdRate`). */
export type ManaRate = ManaUsdRate;

export const USD_WEI_PER_CENT = 10n ** 16n;
export const USD_WEI_PER_CREDIT = BigInt(USD_CENTS_PER_CREDIT) * USD_WEI_PER_CENT;

function toWei(manaWei: string | bigint): ManaWei | null {
  return parseManaWei(manaWei);
}

/** MANA wei to USD wei (1e18 = $1): `wei * rate / 10^decimals`, flooring. */
export function manaWeiToUsdWei(manaWei: string | bigint, { rate, decimals }: ManaRate): bigint {
  const wei = toWei(manaWei);
  if (wei === null) throw new RangeError(`not a MANA amount: ${String(manaWei)}`);
  return (wei * rate) / 10n ** BigInt(decimals);
}

/** USD cents to MANA wei at the rate, rounded UP so a quoted MANA amount never sits below the USD it covers. */
export function usdCentsToManaWei(cents: number, { rate, decimals }: ManaRate): bigint {
  if (!Number.isFinite(cents) || cents <= 0 || rate <= 0n) return 0n;
  const usdWei = BigInt(Math.ceil(cents)) * USD_WEI_PER_CENT;
  const num = usdWei * 10n ** BigInt(decimals);
  return (num + rate - 1n) / rate;
}

/** MANA wei to credits shown, rounded UP and at least 1; null for a malformed amount. */
export function manaWeiToCredits(manaWei: string | bigint, rate: ManaRate): number | null {
  let usdWei: bigint;
  try {
    usdWei = manaWeiToUsdWei(manaWei, rate);
  } catch {
    return null;
  }
  const whole = usdWei / USD_WEI_PER_CREDIT;
  const credits = usdWei % USD_WEI_PER_CREDIT > 0n ? whole + 1n : whole;
  const n = Number(credits);
  return n < 1 ? 1 : n;
}

/** MANA wei to USD cents, rounded UP (model/money `manaUsdCentsCeil`); 0 when it cannot be priced. */
export function manaWeiToUsdCents(manaWei: string | bigint, rate: ManaRate): number {
  const wei = toWei(manaWei);
  if (wei === null) return 0;
  return manaUsdCentsCeil(wei, rate) ?? 0;
}

/**
 * The credit price to DISPLAY for a listing row. A legacy MANA row converts at the live rate
 * (indicative until checkout locks it), a USD-pegged row already carries its credits, and 0 means "no
 * price yet" while the rate loads. A row the payment rule makes MANA-only (a resale, LAND, a rental)
 * has no credit price at all: null.
 */
export function displayCredits(row: { manaWei?: string | null; priceCredits: number; manaOnly?: boolean }, rate: ManaRate | undefined): number | null {
  if (row.manaOnly) return null;
  if (!row.manaWei) return row.priceCredits;
  if (!rate) return 0;
  return manaWeiToCredits(row.manaWei, rate) ?? 0;
}
