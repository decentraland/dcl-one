// from: shop@94584a5c:app/src/lib/currency.ts, app/src/lib/payment-options.ts (manaForRemainder),
//       app/src/lib/api.ts (usdWeiToCents), app/src/lib/mana-rate.ts, app/src/lib/cart-checkout.ts
// Mirrors src/shop/types.rs `UsdCents`, `ManaWei` and `ManaUsdRate`: exact integers, the same rounding
// rules and the same refusals (null where Rust answers None). The pinned vectors are
// tests/fixtures/shop/rounding-vectors.json, checked by money.test.ts as the Rust tests check them.
// Bounds: Rust holds cents in an i64; here they are refused past Number.MAX_SAFE_INTEGER rather than
// rounded. MANA is a bigint bounded to U256 like the Rust type.

import { checkedU256, divCeil, inU256, parseDecimal } from "./decimal";

export type UsdCents = number & { readonly __usdCents: true };
export type ManaWei = bigint & { readonly __manaWei: true };

/** One credit is ten US cents (upstream `USD_CENTS_PER_CREDIT`). */
export const CENTS_PER_CREDIT = 10;
export const WEI_PER_MANA = 10n ** 18n;
const USD_WEI_PER_CENT = 10n ** 16n;
const BPS = 10_000n;
const PPM = 1_000_000n;
export const COUPON_PPM = 1_000_000;

export const ZERO_CENTS = 0 as UsdCents;
export const ZERO_MANA = 0n as ManaWei;

function safeCents(n: number): UsdCents | null {
  return Number.isSafeInteger(n) ? (n as UsdCents) : null;
}

export function usdCents(value: number): UsdCents {
  const cents = safeCents(value);
  if (cents === null) throw new RangeError(`not a whole number of cents: ${value}`);
  return cents;
}

export function parseUsdCents(value: unknown): UsdCents | null {
  return typeof value === "number" ? safeCents(value) : null;
}

export function centsFromCredits(credits: number): UsdCents | null {
  return Number.isSafeInteger(credits) ? safeCents(credits * CENTS_PER_CREDIT) : null;
}

function remEuclid(cents: number): number {
  return ((cents % CENTS_PER_CREDIT) + CENTS_PER_CREDIT) % CENTS_PER_CREDIT;
}

/** Credits for anything the buyer holds: a balance, a held amount (upstream `Math.floor(cents / 10)`). */
export function creditsFloor(cents: UsdCents): number {
  return (cents - remEuclid(cents)) / CENTS_PER_CREDIT;
}

/** Credits for anything the buyer must cover: a price, a charge (upstream `Math.ceil(cents / 10)`). */
export function creditsCeil(cents: UsdCents): number {
  return creditsFloor(cents) + (remEuclid(cents) !== 0 ? 1 : 0);
}

/** What a quoted price is debited at: up to the next whole credit (upstream `chargeForQuote`). */
export function chargeForQuote(quoted: UsdCents): UsdCents | null {
  if (quoted <= 0) return null;
  return safeCents(creditsCeil(quoted) * CENTS_PER_CREDIT);
}

/** The credits leg a balance can fund in a mixed payment: whole credits only. */
export function wholeCreditsFloor(cents: UsdCents): UsdCents {
  return (creditsFloor(cents) * CENTS_PER_CREDIT) as UsdCents;
}

export function addCents(a: UsdCents, b: UsdCents): UsdCents | null {
  return safeCents(a + b);
}

export function subCents(a: UsdCents, b: UsdCents): UsdCents | null {
  return safeCents(a - b);
}

export function mulCents(a: UsdCents, n: number): UsdCents | null {
  return Number.isSafeInteger(n) ? safeCents(a * n) : null;
}

/** Upstream shop `usdWeiToCents`: a USD-pegged amount (1e18 = $1) in cents, rounded up. */
export function centsFromUsdWeiCeil(usdWei: bigint): UsdCents | null {
  if (!inU256(usdWei)) return null;
  const cents = divCeil(usdWei, USD_WEI_PER_CENT);
  return cents <= BigInt(Number.MAX_SAFE_INTEGER) ? (Number(cents) as UsdCents) : null;
}

function keptPpm(discountPpm: number): bigint | null {
  if (!Number.isInteger(discountPpm) || discountPpm < 0 || discountPpm > 0xffff_ffff) return null;
  const ppm = BigInt(discountPpm);
  return PPM - (ppm < PPM ? ppm : PPM);
}

/** Upstream shop `discountedUsdCents`: the price after a rate coupon of `discountPpm`, rounded up. */
export function discountedCents(cents: UsdCents, discountPpm: number): UsdCents | null {
  if (cents <= 0) return cents;
  const kept = keptPpm(discountPpm);
  if (kept === null) return null;
  const result = (BigInt(cents) * kept + PPM - 1n) / PPM;
  return result <= BigInt(Number.MAX_SAFE_INTEGER) ? (Number(result) as UsdCents) : null;
}

export function manaWei(value: bigint | string): ManaWei {
  const parsed = parseManaWei(value);
  if (parsed === null) throw new RangeError(`not a MANA amount in wei: ${String(value)}`);
  return parsed;
}

/** A wire amount (decimal string, as serde writes a U256) or an in-memory bigint. */
export function parseManaWei(value: unknown): ManaWei | null {
  if (typeof value === "bigint") return inU256(value) ? (value as ManaWei) : null;
  const parsed = parseDecimal(value);
  return parsed === null ? null : (parsed as ManaWei);
}

export function manaToWire(value: ManaWei): string {
  return value.toString();
}

export function manaFromWhole(whole: number | bigint): ManaWei {
  return manaWei(BigInt(whole) * WEI_PER_MANA);
}

export function addMana(a: ManaWei, b: ManaWei): ManaWei | null {
  return checkedU256(a + b) as ManaWei | null;
}

export function mulMana(a: ManaWei, n: number): ManaWei | null {
  if (!Number.isSafeInteger(n) || n < 0) return null;
  return checkedU256(a * BigInt(n)) as ManaWei | null;
}

/**
 * The MANA/USD reference rate: the marketplace's `manaUsdAggregator` answer (USD per MANA scaled by
 * 10^decimals). Null for a zero answer or more than 36 decimals, as `ManaUsdRate::new`.
 */
export type ManaUsdRate = { readonly rate: bigint; readonly decimals: number };

export function manaUsdRate(rate: bigint, decimals: number): ManaUsdRate | null {
  const ok = inU256(rate) && rate !== 0n && Number.isInteger(decimals) && decimals >= 0 && decimals <= 36;
  return ok ? { rate, decimals } : null;
}

function pow10(decimals: number): bigint | null {
  if (!Number.isInteger(decimals) || decimals < 0 || decimals > 255) return null;
  return checkedU256(10n ** BigInt(decimals));
}

/**
 * Upstream `oracle.usdCentsToManaWei`: `cents * 1e18 / 100`, times `10^decimals / rate`, then
 * `(10000 + bufferBps) / 10000`, each step flooring. Null where upstream throws or on overflow.
 */
export function manaFromUsdCents(cents: UsdCents, rate: bigint, decimals: number, bufferBps: number): ManaWei | null {
  if (cents <= 0 || rate <= 0n || !inU256(rate)) return null;
  if (!Number.isInteger(bufferBps) || bufferBps < 0 || bufferBps > 0xffff_ffff) return null;
  const usdWei = checkedU256(BigInt(cents) * WEI_PER_MANA);
  const scale = pow10(decimals);
  if (usdWei === null || scale === null) return null;
  const scaled = checkedU256((usdWei / 100n) * scale);
  if (scaled === null) return null;
  const buffered = checkedU256((scaled / rate) * (BPS + BigInt(bufferBps)));
  return buffered === null ? null : ((buffered / BPS) as ManaWei);
}

/** Upstream shop `manaWeiToUsdCents`: USD wei `= wei * rate / 10^decimals` (flooring), cents up. */
export function manaUsdCentsCeil(wei: ManaWei, rate: ManaUsdRate): UsdCents | null {
  const scale = pow10(rate.decimals);
  const product = checkedU256(wei * rate.rate);
  if (scale === null || product === null) return null;
  return centsFromUsdWeiCeil(product / scale);
}

/** Upstream shop `readTradeManaPriceWei`: the MANA a USD-pegged amount settles for, flooring. */
export function manaFromUsdWei(usdWei: bigint, rate: ManaUsdRate): ManaWei | null {
  const scale = pow10(rate.decimals);
  const product = scale === null ? null : checkedU256(usdWei * scale);
  return product === null ? null : ((product / rate.rate) as ManaWei);
}

/** Upstream shop `discountedManaWei`: the amount after a rate coupon, rounded up. */
export function discountedMana(wei: ManaWei, discountPpm: number): ManaWei | null {
  const kept = keptPpm(discountPpm);
  const product = kept === null ? null : checkedU256(wei * kept);
  return product === null ? null : (divCeil(product, PPM) as ManaWei);
}

/** Upstream `applyCapHeadroom`: `quoted * pct / 100`, flooring. */
export function withCapHeadroom(wei: ManaWei, headroomPct: number): ManaWei | null {
  if (!Number.isInteger(headroomPct) || headroomPct < 0) return null;
  const product = checkedU256(wei * BigInt(headroomPct));
  return product === null ? null : ((product / 100n) as ManaWei);
}

/**
 * Upstream shop `manaForRemainder`: the MANA that covers `remainder` of a purchase whose whole `price`
 * costs `priceWei`, rounded up so a mixed payment never under-funds the price.
 */
export function manaForRemainder(remainder: UsdCents, price: UsdCents, priceWei: ManaWei): ManaWei | null {
  if (remainder <= 0 || price <= 0) return ZERO_MANA;
  if (remainder >= price) return priceWei;
  const numerator = checkedU256(priceWei * BigInt(remainder));
  return numerator === null ? null : (divCeil(numerator, BigInt(price)) as ManaWei);
}

/**
 * Exact MANA: every significant digit of the 18 decimals, trailing zeros dropped ("2.3", "0.000000000000000001").
 * A displayed conversion is never the charge, so nothing here rounds.
 */
export function formatMana(wei: ManaWei, opts: { group?: boolean; decimals?: number } = {}): string {
  const whole = wei / WEI_PER_MANA;
  let fraction = (wei % WEI_PER_MANA).toString().padStart(18, "0");
  if (opts.decimals != null) fraction = fraction.slice(0, opts.decimals);
  fraction = fraction.replace(/0+$/, "");
  const wholeText = opts.group ? whole.toLocaleString("en-US") : whole.toString();
  return fraction ? `${wholeText}.${fraction}` : wholeText;
}

/** "$12.34" from exact cents. */
export function formatUsd(cents: UsdCents): string {
  const sign = cents < 0 ? "-" : "";
  const abs = Math.abs(cents);
  return `${sign}$${Math.floor(abs / 100).toLocaleString("en-US")}.${String(abs % 100).padStart(2, "0")}`;
}
