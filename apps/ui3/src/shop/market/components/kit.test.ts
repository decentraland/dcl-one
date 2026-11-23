import { beforeEach, describe, expect, it } from "vitest";

import { MetaTransactionError } from "../lib/metaTx";
import { GaslessCancelFailedError } from "../lib/trades";
import { writeFailureText } from "./kit";

describe("when a market write fails", () => {
  describe("and the relay refused it for its request limit", () => {
    let error: MetaTransactionError;

    beforeEach(() => {
      error = new MetaTransactionError("Too many requests. Retry in 2 seconds.", "api_limit_reached", 429, true, { error: "api_limit_reached", retryAt: 1 });
    });

    it("should tell the seller to wait a few seconds and try again", () => {
      expect(writeFailureText(error)).toBe("Too many requests from your connection. Wait a few seconds and try again.");
    });
  });

  describe("and the relay refused it because the account's fee-free quota is spent", () => {
    let error: MetaTransactionError;

    beforeEach(() => {
      error = new MetaTransactionError("Max amount of transactions reached", "quota_reached", 429, true, { error: "quota_reached", quota: 30 });
    });

    it("should name the spent quota", () => {
      expect(writeFailureText(error)).toBe("You've used all 30 fee-free transactions this account gets in 24 hours. Try again later.");
    });
  });

  describe("and the relay refused it because the shared fee-free limit is spent", () => {
    let error: MetaTransactionError;

    beforeEach(() => {
      error = new MetaTransactionError("Budget spent", "relay_budget_spent", 429, true, { error: "relay_budget_spent" });
    });

    it("should say the shared fee-free limit was reached", () => {
      expect(writeFailureText(error)).toBe("The shared fee-free limit for this operation has been reached. Try again later.");
    });
  });

  describe("and a gasless-only cancellation failed because the account's fee-free quota is spent", () => {
    let error: GaslessCancelFailedError;

    beforeEach(() => {
      error = new GaslessCancelFailedError(new MetaTransactionError("Max amount of transactions reached", "quota_reached", 429, true, { error: "quota_reached", quota: 30 }));
    });

    it("should name the spent quota", () => {
      expect(writeFailureText(error)).toBe("You've used all 30 fee-free transactions this account gets in 24 hours. Try again later.");
    });
  });

  describe("and a gasless-only cancellation failed because the shared fee-free limit is spent", () => {
    let error: GaslessCancelFailedError;

    beforeEach(() => {
      error = new GaslessCancelFailedError(new MetaTransactionError("Budget spent", "relay_budget_spent", 429, true, { error: "relay_budget_spent" }));
    });

    it("should say the shared fee-free limit was reached", () => {
      expect(writeFailureText(error)).toBe("The shared fee-free limit for this operation has been reached. Try again later.");
    });
  });

  describe("and a gasless-only cancellation failed for another reason", () => {
    let error: GaslessCancelFailedError;

    beforeEach(() => {
      error = new GaslessCancelFailedError(new MetaTransactionError("relay down", "UNKNOWN", 503, true, {}));
    });

    it("should say the gasless cancellation was not confirmed", () => {
      expect(writeFailureText(error)).toBe("The gasless cancellation was not confirmed");
    });
  });

  describe("and the relay refused it for another reason", () => {
    let error: MetaTransactionError;

    beforeEach(() => {
      error = new MetaTransactionError("relay down", "UNKNOWN", 503, true, {});
    });

    it("should show the relay's own message", () => {
      expect(writeFailureText(error)).toBe("relay down");
    });
  });
});
