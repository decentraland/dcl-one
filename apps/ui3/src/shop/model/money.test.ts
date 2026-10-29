import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import {
  addMana,
  centsFromCredits,
  centsFromUsdWeiCeil,
  chargeForQuote,
  creditsCeil,
  creditsFloor,
  discountedCents,
  discountedMana,
  formatMana,
  formatUsd,
  manaForRemainder,
  manaFromUsdCents,
  manaFromUsdWei,
  manaFromWhole,
  manaUsdCentsCeil,
  manaUsdRate,
  manaWei,
  parseManaWei,
  usdCents,
  wholeCreditsFloor,
  withCapHeadroom,
  type UsdCents,
} from "./money";
import { U256_MAX } from "./decimal";

type Vectors = Record<string, { cases: unknown[]; refused?: unknown[] }>;

const vectors = JSON.parse(
  readFileSync(join(__dirname, "../../../../../services/catalyst/tests/fixtures/shop/rounding-vectors.json"), "utf8"),
) as Vectors;

function pairs(name: string): [number, number][] {
  return vectors[name]!.cases as [number, number][];
}

function rows(name: string): Record<string, string | number>[] {
  return vectors[name]!.cases as Record<string, string | number>[];
}

const c = (n: number | string) => usdCents(Number(n));
const rateOf = (row: Record<string, string | number>) => manaUsdRate(BigInt(row.rate!), Number(row.decimals))!;

describe("rounding matches the pinned upstream vectors (tests/fixtures/shop/rounding-vectors.json)", () => {
  it("chargeForQuote", () => {
    for (const [quoted, charged] of pairs("chargeForQuote")) expect(chargeForQuote(c(quoted)), `${quoted}`).toBe(charged);
    for (const quoted of vectors.chargeForQuote!.refused as number[]) expect(chargeForQuote(c(quoted))).toBeNull();
  });

  it("creditsCeil and creditsFloor", () => {
    for (const [cents, credits] of pairs("creditsCeil")) expect(creditsCeil(c(cents)), `ceil ${cents}`).toBe(credits);
    for (const [cents, credits] of pairs("creditsFloor")) expect(creditsFloor(c(cents)), `floor ${cents}`).toBe(credits);
  });

  it("wholeCreditsFloor", () => {
    for (const [cents, leg] of pairs("wholeCreditsFloor")) expect(wholeCreditsFloor(c(cents)), `leg ${cents}`).toBe(leg);
  });

  it("usdCentsToManaWei", () => {
    for (const row of rows("usdCentsToManaWei")) {
      const got = manaFromUsdCents(c(row.cents!), BigInt(row.rate!), Number(row.decimals), Number(row.bufferBps));
      expect(got, JSON.stringify(row)).toBe(BigInt(row.manaWei!));
    }
    for (const row of vectors.usdCentsToManaWei!.refused as Record<string, string | number>[]) {
      expect(manaFromUsdCents(c(row.cents!), BigInt(row.rate!), 8, 200), JSON.stringify(row)).toBeNull();
    }
  });

  it("capHeadroom", () => {
    for (const row of rows("capHeadroom")) {
      expect(withCapHeadroom(manaWei(String(row.quotedWei)), Number(row.pct))).toBe(BigInt(row.capWei!));
    }
  });

  it("manaForRemainder", () => {
    for (const row of rows("manaForRemainder")) {
      const got = manaForRemainder(c(row.remainder!), c(row.price!), manaWei(String(row.priceWei)));
      expect(got, JSON.stringify(row)).toBe(BigInt(row.wei!));
    }
  });

  it("usdWeiToCents", () => {
    for (const [wei, cents] of vectors.usdWeiToCents!.cases as [string, number][]) {
      expect(centsFromUsdWeiCeil(BigInt(wei)), wei).toBe(cents);
    }
  });

  it("manaWeiToUsdCents", () => {
    for (const row of rows("manaWeiToUsdCents")) {
      expect(manaUsdCentsCeil(manaWei(String(row.manaWei)), rateOf(row)), JSON.stringify(row)).toBe(row.cents);
    }
  });

  it("usdWeiToManaWei", () => {
    for (const row of rows("usdWeiToManaWei")) {
      expect(manaFromUsdWei(BigInt(row.usdWei!), rateOf(row))).toBe(BigInt(row.manaWei!));
    }
  });

  it("discountedUsdCents", () => {
    for (const [cents, ppm, want] of vectors.discountedUsdCents!.cases as [number, number, number][]) {
      expect(discountedCents(c(cents), ppm), `${cents} ${ppm}`).toBe(want);
    }
  });

  it("the rate and credit constructors", () => {
    expect(manaUsdRate(0n, 8), "a non-positive answer is no rate").toBeNull();
    expect(manaUsdRate(1n, 37)).toBeNull();
    expect(centsFromCredits(41)).toBe(410);
  });
});

describe("the policy test numbers (tests/shop/policy.rs quotes_come_from_the_offer_and_the_rate_only)", () => {
  const half = manaUsdRate(50_000_000n, 8)!;

  it("a store mint of 2.3 MANA is 115c at $0.50, the emote's 4.02 MANA is 201c", () => {
    expect(manaUsdCentsCeil(manaWei(2_300n * 10n ** 15n), half)).toBe(115);
    expect(manaUsdCentsCeil(manaWei(4_020n * 10n ** 15n), half)).toBe(201);
    expect(manaUsdCentsCeil(manaFromWhole(100), half), "100 MANA at $0.50").toBe(5000);
  });

  it("$1 is 2 MANA at $0.50, a 30% coupon leaves 70c and 1.4 MANA", () => {
    const pegged = manaFromUsdWei(10n ** 18n, half)!;
    expect(pegged).toBe(manaFromWhole(2));
    expect(discountedCents(c(100), 300_000)).toBe(70);
    expect(discountedMana(pegged, 300_000)).toBe(1_400n * 10n ** 15n);
    expect(centsFromUsdWeiCeil(1_234_500_000_000_000_000n), "cents round up").toBe(124);
  });

  it("the group charge sums each line's whole-credit charge: 115 + 201 quoted, 120 + 210 charged", () => {
    expect((chargeForQuote(c(115))! + chargeForQuote(c(201))!) as UsdCents).toBe(330);
  });

  it("316c at $0.50 plus the 2% buffer is 6.4464 MANA", () => {
    expect(manaFromUsdCents(c(316), 50_000_000n, 8, 200)).toBe(6_446_400n * 10n ** 12n);
  });
});

describe("bounds and formatting", () => {
  it("refuses what Rust refuses", () => {
    expect(parseManaWei("-1")).toBeNull();
    expect(parseManaWei("01")).toBe(1n);
    expect(parseManaWei((U256_MAX + 1n).toString())).toBeNull();
    expect(addMana(manaWei(U256_MAX), manaWei(1n))).toBeNull();
    expect(() => usdCents(1.5)).toThrow();
    expect(chargeForQuote(c(Number.MAX_SAFE_INTEGER))).toBeNull();
  });

  it("MANA is shown in exact units", () => {
    expect(formatMana(manaWei(2_300n * 10n ** 15n))).toBe("2.3");
    expect(formatMana(manaWei(1n))).toBe("0.000000000000000001");
    expect(formatMana(manaFromWhole(4000), { group: true })).toBe("4,000");
    expect(formatMana(manaWei(6_799_999_999_999_999_999n))).toBe("6.799999999999999999");
    expect(formatUsd(c(12_345))).toBe("$123.45");
  });
});
