import { useQueryClient, type QueryClient } from "@tanstack/react-query";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { SHOP_QUERY_KEYS } from "../../checkout/lib/after-purchase";
import { createDemo, DemoShell } from "../../checkout/demo";
import { TRADE_ASSET, type Trade } from "../../market/lib/trades";
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

describe("LandTradeBuy", () => {
  it("a bought LAND trade has the page read its MANA balance again", async () => {
    const demo = createDemo({ session: "web3" });
    render(
      <DemoShell demo={demo} path="/shop/lands">
        <QueryProbe />
        <LandTradeBuy name="Parcel 1,2" tradeId="trade-1" quoted={PRICE as ManaWei} onClose={() => {}} loadTrade={async () => trade} />
      </DemoShell>,
    );
    const balance = [SHOP_QUERY_KEYS.manaBalance, BOOK.buyer.toLowerCase(), 137];
    act(() => void pageQueries!.setQueryData(balance, 10n ** 21n));
    expect(pageQueries!.getQueryState(balance)?.isInvalidated).toBe(false);

    // Enabled once the listing and the wallet's network are read.
    await vi.waitFor(() => expect(screen.getByTestId("order-buy")).toBeEnabled());
    await act(async () => fireEvent.click(screen.getByTestId("order-buy")));
    await vi.waitFor(() => expect(pageQueries!.getQueryState(balance)?.isInvalidated).toBe(true));
  });
});
