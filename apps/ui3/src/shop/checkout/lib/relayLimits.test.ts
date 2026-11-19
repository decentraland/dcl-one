// The relay's daily limits as the buyer reads them (lib/errors relayLimitText, through friendlyError): the account's
// fee-free quota, with when it frees up, or the relay's own pause; any other refusal keeps its own copy.
import { describe, expect, it } from "vitest";
import { GaslessUnavailableError } from "../chain/metatx";
import { MetaTransactionError } from "../../market/lib/metaTx";
import { friendlyError, relayLimitText } from "./errors";

const NOW = new Date(2026, 10, 19, 10, 0, 0);
const at = (hours: number, minutes = 0) => new Date(2026, 10, 19, hours, minutes, 0);
const time = (d: Date) => new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit" }).format(d);
const quota = (refusal: Record<string, unknown>) => new GaslessUnavailableError("Max amount of transactions reached", "relayer-rejected", { error: "quota_reached", ...refusal });

describe("the relay's daily limits, said to the buyer", () => {
  it("names the spent quota, its size and the time it frees up today", () => {
    const retryAt = Math.floor(at(14, 5).getTime() / 1000);
    expect(relayLimitText(quota({ quota: 30, retryAt }), NOW)).toBe(`You've used all 30 of today's fee-free transactions for this account. You can buy fee-free again at ${time(at(14, 5))}.`);
  });

  it("says tomorrow for a time past midnight, and within 24 hours without a usable time", () => {
    const tomorrow = new Date(2026, 10, 20, 9, 30, 0);
    expect(relayLimitText(quota({ retryAt: Math.floor(tomorrow.getTime() / 1000) }), NOW)).toBe(`You've used all of today's fee-free transactions for this account. You can buy fee-free again tomorrow at ${time(tomorrow)}.`);
    for (const retryAt of [undefined, Math.floor(at(9).getTime() / 1000), Math.floor(NOW.getTime() / 1000) + 2 * 86_400]) {
      expect(relayLimitText(quota({ retryAt }), NOW)).toBe("You've used all of today's fee-free transactions for this account. You can buy fee-free again within 24 hours.");
    }
  });

  it("says the relay's own pause", () => {
    expect(relayLimitText(new GaslessUnavailableError("spent", "relayer-rejected", { error: "relay_budget_spent" }), NOW)).toBe("Fee-free checkout is paused for today.");
  });

  it("reads the marketplace client's refusals the same way", () => {
    const refused = new MetaTransactionError("Max amount of transactions reached", "quota_reached", 429, true, { error: "quota_reached", quota: 30 });
    expect(relayLimitText(refused, NOW)).toMatch(/^You've used all 30 of today's fee-free transactions/);
  });

  it("leaves every other refusal to its own copy, and friendlyError says the limits first", () => {
    expect(relayLimitText(new GaslessUnavailableError("execution reverted", "relayer-rejected", { error: "invalid_transaction" }), NOW)).toBeNull();
    expect(relayLimitText(new Error("boom"), NOW)).toBeNull();
    expect(relayLimitText(null, NOW)).toBeNull();
    expect(friendlyError(new GaslessUnavailableError("spent", "relayer-rejected", { error: "relay_budget_spent" }), "fallback")).toBe("Fee-free checkout is paused for today.");
    expect(friendlyError(new GaslessUnavailableError("execution reverted", "relayer-rejected", { error: "invalid_transaction" }), "fallback")).toBe("fallback");
  });
});
