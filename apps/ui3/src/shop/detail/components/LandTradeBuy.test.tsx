import { useQueryClient, type QueryClient } from "@tanstack/react-query";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { SHOP_QUERY_KEYS } from "../../checkout/lib/after-purchase";
import { createDemo, DemoShell } from "../../checkout/demo";
import { TxPendingError } from "../../market/lib/tx";
import { TRADE_ASSET, type Trade } from "../../market/lib/trades";
import { buyTradeWithMana } from "../../market/purchase/executeOrder";
import type { ManaWei } from "@shop/model/money";
import { BOOK } from "@shop/payment/fixtures";
import LandTradeBuy from "./LandTradeBuy";

// The wallet's side of the trade is not under test here: it settles at once with one transaction.
vi.mock("../../market/purchase/executeOrder", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../market/purchase/executeOrder")>()),
  buyTradeWithMana: vi.fn(async () => `0x${"e".repeat(64)}`),
}));

/** The page's query client, as the dialog's own page holds it. */
let pageQueries: QueryClient | null = null;
function QueryProbe() {
  pageQueries = useQueryClient();
  return null;
}

const PRICE = 10n ** 18n;
/** A LAND listing priced in MANA on Polygon: only what the dialog reads of it. */
const trade = { id: "trade-1", contract: `0x${"4".repeat(40)}`, chainId: 137, sent: [], received: [{ assetType: TRADE_ASSET.ERC20, amount: PRICE.toString() }] } as unknown as Trade;
const LAND = { contractAddress: `0x${"5".repeat(40)}`, tokenId: "77" };
const balance = [SHOP_QUERY_KEYS.manaBalance, BOOK.buyer.toLowerCase(), 137];
const thisLand = ["owned-token", LAND.contractAddress, LAND.tokenId, BOOK.buyer.toLowerCase()];
const otherToken = ["owned-token", `0x${"6".repeat(40)}`, "9", BOOK.buyer.toLowerCase()];

/** Opens the dialog over a page holding a MANA balance and two owned tokens, and presses Buy. */
async function buy(land?: typeof LAND) {
  const demo = createDemo({ session: "web3" });
  render(
    <DemoShell demo={demo} path="/shop/lands">
      <QueryProbe />
      <LandTradeBuy name="Parcel 1,2" tradeId="trade-1" quoted={PRICE as ManaWei} onClose={() => {}} loadTrade={async () => trade} land={land} />
    </DemoShell>,
  );
  act(() => {
    pageQueries!.setQueryData(balance, 10n ** 21n);
    pageQueries!.setQueryData(thisLand, null);
    pageQueries!.setQueryData(otherToken, null);
  });
  expect(pageQueries!.getQueryState(balance)?.isInvalidated).toBe(false);
  // Enabled once the listing and the wallet's network are read.
  await vi.waitFor(() => expect(screen.getByTestId("order-buy")).toBeEnabled());
  await act(async () => fireEvent.click(screen.getByTestId("order-buy")));
}

const invalidated = (key: unknown[]) => pageQueries!.getQueryState(key)?.isInvalidated;

describe("LandTradeBuy", () => {
  it("a bought LAND trade has the page read its MANA balance again", async () => {
    await buy();
    await vi.waitFor(() => expect(invalidated(balance)).toBe(true));
  });

  it("a bought LAND trade reads that LAND again, and no other token", async () => {
    await buy(LAND);
    await vi.waitFor(() => expect(invalidated(balance)).toBe(true));
    expect(invalidated(thisLand)).toBe(true);
    expect(invalidated(otherToken)).toBe(false);
  });

  it("a LAND trade still confirming reads the balance and that LAND again too", async () => {
    vi.mocked(buyTradeWithMana).mockRejectedValueOnce(new TxPendingError(`0x${"d".repeat(64)}`, "The purchase"));
    await buy(LAND);
    await screen.findByText(/still confirming/);
    expect(invalidated(balance)).toBe(true);
    expect(invalidated(thisLand)).toBe(true);
    expect(invalidated(otherToken)).toBe(false);
  });

  it("a LAND trade sent and then lost track of reads the balance again", async () => {
    vi.mocked(buyTradeWithMana).mockImplementationOnce(async (opts) => {
      opts.onSubmitted?.(`0x${"c".repeat(64)}`);
      throw new Error("the receipt read failed");
    });
    await buy(LAND);
    await screen.findByText(/still confirming/);
    expect(invalidated(balance)).toBe(true);
  });

  it("a declined LAND trade spent nothing and reads nothing again", async () => {
    vi.mocked(buyTradeWithMana).mockRejectedValueOnce(new Error("User rejected the request."));
    await buy(LAND);
    await screen.findByText(/Nothing was bought/);
    expect(invalidated(balance)).toBe(false);
  });
});
