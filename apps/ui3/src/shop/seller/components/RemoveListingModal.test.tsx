import { fireEvent, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { BOOK } from "@shop/payment/fixtures";
import { MANA, nftRow } from "../../market/fixtures";
import { MetaTransactionError } from "../../market/lib/metaTx";
import { GaslessCancelFailedError } from "../../market/lib/trades";
import { fakeWallet } from "../../market/testing";
import { mountShop } from "../../market/testHarness";

const relay = vi.hoisted(() => ({ cancelTrade: vi.fn() }));

vi.mock("../../market/lib/trades", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../market/lib/trades")>();
  return { ...actual, cancelTrade: relay.cancelTrade, fetchTrade: vi.fn(async () => ({ id: "old-1" })) };
});

describe("when the free cancellation of a listing is refused", () => {
  let wallet: ReturnType<typeof fakeWallet>;

  const confirm = async () => {
    mountShop({
      path: `/shop/token/${BOOK.collection}/105312/cancel`,
      who: "web3",
      wallet,
      routes: [
        {
          method: "GET",
          path: /^\/v1\/nfts$/,
          reply: () => ({ body: { data: [nftRow({ tokenId: "105312", itemId: "7", name: "Neon Jacket", owner: wallet.address, onSale: { price: MANA(1), tradeId: "old-1" } })], total: 1 } }),
        },
      ],
    });
    fireEvent.click(await screen.findByRole("button", { name: "Confirm" }));
  };

  beforeEach(() => {
    wallet = fakeWallet();
  });

  afterEach(() => {
    relay.cancelTrade.mockReset();
  });

  describe("and the refusal is the relay's request limit", () => {
    beforeEach(async () => {
      relay.cancelTrade.mockRejectedValueOnce(
        new GaslessCancelFailedError(new MetaTransactionError("Too many requests", "api_limit_reached", 429, true, { error: "api_limit_reached" })),
      );
      await confirm();
    });

    it("should report the limit and not offer the paid cancellation", async () => {
      expect(await screen.findByText("Too many requests from your connection. Wait a few seconds and try again.")).toBeInTheDocument();
      expect(screen.queryByTestId("shop-gasless-failed")).toBeNull();
    });
  });

  describe("and the refusal is the account's daily quota", () => {
    beforeEach(async () => {
      relay.cancelTrade.mockRejectedValueOnce(
        new GaslessCancelFailedError(new MetaTransactionError("Quota reached", "quota_reached", 429, true, { error: "quota_reached", quota: 5 })),
      );
      await confirm();
    });

    it("should show the quota and offer the paid cancellation", async () => {
      expect(await screen.findByTestId("shop-gasless-failed")).toBeInTheDocument();
      expect(screen.getByText(/You've used all 5 fee-free transactions this account gets in 24 hours\./)).toBeInTheDocument();
    });
  });
});
