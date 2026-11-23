// The relay's own limits as the buyer reads them (lib/errors relayLimitText, through friendlyError): the account's
// fee-free quota over its rolling 24 hours, with when it frees up, or the relay's shared limit on fee-free operations
// that aren't sales; any other refusal keeps its own copy.
import { describe, expect, it } from "vitest";
import { GaslessUnavailableError } from "../chain/metatx";
import { MetaTransactionError } from "../../market/lib/metaTx";
import { friendlyError, isRequestLimited, relayLimitText } from "./errors";

const NOW = new Date(2026, 10, 19, 10, 0, 0);
const at = (hours: number, minutes = 0, seconds = 0) => new Date(2026, 10, 19, hours, minutes, seconds);
const time = (d: Date) => new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit" }).format(d);
const secs = (d: Date) => Math.floor(d.getTime() / 1000);
const quota = (refusal: Record<string, unknown>) => new GaslessUnavailableError("Max amount of transactions reached", "relayer-rejected", { error: "quota_reached", ...refusal });

describe("the relay's limits, said to the buyer", () => {
  it("names the spent quota, its size and the minute it frees up, never too early", () => {
    expect(relayLimitText(quota({ quota: 30, retryAt: secs(at(14, 5)) }), NOW)).toBe(`You've used all 30 fee-free transactions this account gets in 24 hours. Try again at ${time(at(14, 5))}.`);
    // 14:05:20 frees up after 14:05: the time shown is the next minute.
    expect(relayLimitText(quota({ quota: 30, retryAt: secs(at(14, 5, 20)) }), NOW)).toBe(`You've used all 30 fee-free transactions this account gets in 24 hours. Try again at ${time(at(14, 6))}.`);
  });

  it("says tomorrow for a time past midnight, and to try again later without a usable time", () => {
    const tomorrow = new Date(2026, 10, 20, 9, 30, 0);
    expect(relayLimitText(quota({ retryAt: secs(tomorrow) }), NOW)).toBe(`You've used all the fee-free transactions this account gets in 24 hours. Try again tomorrow at ${time(tomorrow)}.`);
    for (const retryAt of [undefined, secs(at(9)), secs(NOW) + 2 * 86_400, Number.MAX_SAFE_INTEGER, 1.5, "soon"]) {
      expect(relayLimitText(quota({ retryAt }), NOW), String(retryAt)).toBe("You've used all the fee-free transactions this account gets in 24 hours. Try again later.");
    }
  });

  it("names the date when the rounded time lands two days out, never a day early", () => {
    // Refused at 23:59:45; the slot frees at 23:59:30 the next day, which rounds up to midnight the day after.
    const late = new Date(2026, 10, 19, 23, 59, 45);
    const freed = new Date(2026, 10, 20, 23, 59, 30);
    const shown = new Date(2026, 10, 21, 0, 0, 0);
    const day = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" }).format(shown);
    expect(relayLimitText(quota({ quota: 30, retryAt: secs(freed) }), late)).toBe(`You've used all 30 fee-free transactions this account gets in 24 hours. Try again on ${day} at ${time(shown)}.`);
  });

  it("says an account with a quota of none has no fee-free transactions, rather than promising one", () => {
    expect(relayLimitText(quota({ quota: 0, retryAt: secs(at(14)) }), NOW)).toBe("Fee-free transactions are currently unavailable for this account.");
  });

  it("says the relay's shared limit on operations that aren't sales, and when it frees up when the relay says so", () => {
    expect(relayLimitText(new GaslessUnavailableError("spent", "relayer-rejected", { error: "relay_budget_spent" }), NOW)).toBe("The shared fee-free limit for this operation has been reached. Try again later.");
    expect(relayLimitText(new GaslessUnavailableError("spent", "relayer-rejected", { error: "relay_budget_spent", retryAt: secs(at(11, 30, 1)) }), NOW)).toBe(`The shared fee-free limit for this operation has been reached. Try again at ${time(at(11, 31))}.`);
    expect(relayLimitText(new GaslessUnavailableError("spent", "relayer-rejected", { error: "relay_budget_spent", retryAt: Number.MAX_SAFE_INTEGER }), NOW)).toBe("The shared fee-free limit for this operation has been reached. Try again later.");
  });

  it("says a connection over the relay's request limit to wait a few seconds, without a clock time", () => {
    const limited = new GaslessUnavailableError("Too many requests", "relayer-rejected", { error: "api_limit_reached", retryAt: secs(NOW) + 2 });
    expect(relayLimitText(limited, NOW)).toBe("Too many requests from your connection. Wait a few seconds and try again.");
    expect(friendlyError(limited, "fallback")).toBe("Too many requests from your connection. Wait a few seconds and try again.");
  });

  it("tells the per-connection request limit apart from every other refusal, in both relay clients", () => {
    expect(isRequestLimited(new GaslessUnavailableError("Too many requests", "relayer-rejected", { error: "api_limit_reached" }))).toBe(true);
    expect(isRequestLimited(new MetaTransactionError("Too many requests", "api_limit_reached", 429, true, { error: "api_limit_reached" }))).toBe(true);
    expect(isRequestLimited(quota({ quota: 30 }))).toBe(false);
    expect(isRequestLimited(new GaslessUnavailableError("spent", "relayer-rejected", { error: "relay_budget_spent" }))).toBe(false);
    expect(isRequestLimited(new Error("boom"))).toBe(false);
    expect(isRequestLimited(null)).toBe(false);
  });

  it("reads the marketplace client's refusals the same way", () => {
    const refused = new MetaTransactionError("Max amount of transactions reached", "quota_reached", 429, true, { error: "quota_reached", quota: 30 });
    expect(relayLimitText(refused, NOW)).toBe("You've used all 30 fee-free transactions this account gets in 24 hours. Try again later.");
  });

  it("leaves every other refusal to its own copy, and friendlyError says the limits first", () => {
    expect(relayLimitText(new GaslessUnavailableError("execution reverted", "relayer-rejected", { error: "invalid_transaction" }), NOW)).toBeNull();
    expect(relayLimitText(new Error("boom"), NOW)).toBeNull();
    expect(relayLimitText(null, NOW)).toBeNull();
    expect(friendlyError(new GaslessUnavailableError("spent", "relayer-rejected", { error: "relay_budget_spent" }), "fallback")).toBe("The shared fee-free limit for this operation has been reached. Try again later.");
    expect(friendlyError(new GaslessUnavailableError("execution reverted", "relayer-rejected", { error: "invalid_transaction" }), "fallback")).toBe("fallback");
  });
});
