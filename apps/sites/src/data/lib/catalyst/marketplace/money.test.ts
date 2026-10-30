import { describe, expect, it } from "vitest";

import { weiToMana, weiToManaOrNull } from "./money";

describe("weiToMana \u{2014} returns 0 on garbage", () => {
  it("converts wei strings exactly, passes zero and negatives through numerically, and yields 0 for garbage", () => {
    expect(weiToMana("1234567800000000000")).toBe(1.2345678);
    expect(weiToMana("1000000000000000000000")).toBe(1000);
    expect(weiToMana("0")).toBe(0);
    expect(weiToMana("-1000000000000000000")).toBe(-1);
    expect(weiToMana("abc")).toBe(0);
    expect(weiToMana("")).toBe(0);
  });
});

describe("weiToManaOrNull \u{2014} null-safe display semantics", () => {
  it("converts positive amounts and is null for null / undefined / empty / zero / negatives / garbage", () => {
    expect(weiToManaOrNull("1234567800000000000")).toBe(1.2345678);
    expect(weiToManaOrNull(null)).toBeNull();
    expect(weiToManaOrNull(undefined)).toBeNull();
    expect(weiToManaOrNull("")).toBeNull();
    expect(weiToManaOrNull("0")).toBeNull();
    expect(weiToManaOrNull("-1000000000000000000")).toBeNull();
    expect(weiToManaOrNull("abc")).toBeNull();
  });
});
