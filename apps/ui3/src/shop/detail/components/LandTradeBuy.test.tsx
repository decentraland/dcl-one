import { useQueryClient, type QueryClient } from "@tanstack/react-query";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { SHOP_QUERY_KEYS } from "../../checkout/lib/after-purchase";
import { createDemo, DemoShell } from "../../checkout/demo";
import { APPROVAL_STILL_PENDING, TxPendingError } from "../../market/lib/tx";
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
/** The LAND on sale, its contract in mixed case as a listing may name it; the pages key it in lowercase. */
const LAND = { contractAddress: `0x${"aB".repeat(20)}`, tokenId: "77" };
const contract = LAND.contractAddress.toLowerCase();
const seller = `0x${"7".repeat(40)}`;
const buyer = BOOK.buyer.toLowerCase();
const SALE = `0x${"d".repeat(64)}`;

/** What the page holds when Buy is pressed: what a purchase of this LAND makes stale, and what it does not. */
const stale = {
  balance: [SHOP_QUERY_KEYS.manaBalance, buyer, 137],
  owned: ["owned-token", contract, LAND.tokenId, buyer],
  landPage: ["land-token", contract, LAND.tokenId],
  landOrder: ["land-token-order", contract, LAND.tokenId, seller],
};
const untouched = {
  otherParcel: ["land-token", contract, "78"],
  otherParcelOrder: ["land-token-order", contract, "78", seller],
  otherToken: ["owned-token", `0x${"6".repeat(40)}`, "9", buyer],
};

/** Opens the dialog over a page holding those queries, and presses Buy. */
async function buy(land?: typeof LAND) {
  const demo = createDemo({ session: "web3" });
  render(
    <DemoShell demo={demo} path="/shop/lands">
      <QueryProbe />
      <LandTradeBuy name="Parcel 1,2" tradeId="trade-1" quoted={PRICE as ManaWei} onClose={() => {}} loadTrade={async () => trade} land={land} />
    </DemoShell>,
  );
  act(() => {
    for (const key of [...Object.values(stale), ...Object.values(untouched)]) pageQueries!.setQueryData(key, null);
  });
  // Enabled once the listing and the wallet's network are read.
  await vi.waitFor(() => expect(screen.getByTestId("order-buy")).toBeEnabled());
  await act(async () => fireEvent.click(screen.getByTestId("order-buy")));
}

const invalidated = (keys: Record<string, unknown[]>) =>
  Object.fromEntries(Object.entries(keys).map(([name, key]) => [name, pageQueries!.getQueryState(key)?.isInvalidated]));
const all = (keys: Record<string, unknown[]>, value: boolean) => Object.fromEntries(Object.keys(keys).map((name) => [name, value]));

/** The sale went out (its hash reached `onSubmitted`) and then its wait ran out. */
const saleStillConfirming = async (opts: { onSubmitted?: (hash: string) => void }) => {
  opts.onSubmitted?.(SALE);
  throw new TxPendingError(SALE, "The purchase");
};

describe("LandTradeBuy", () => {
  it("a bought LAND trade has the page read its MANA balance again", async () => {
    await buy();
    await vi.waitFor(() => expect(invalidated({ balance: stale.balance })).toEqual({ balance: true }));
  });

  it("a bought LAND trade reads that LAND again, and no other parcel or token", async () => {
    await buy(LAND);
    await vi.waitFor(() => expect(invalidated(stale)).toEqual(all(stale, true)));
    expect(invalidated(untouched)).toEqual(all(untouched, false));
  });

  it("a sale still confirming reads the balance and that LAND again too, and no other parcel", async () => {
    vi.mocked(buyTradeWithMana).mockImplementationOnce(saleStillConfirming);
    await buy(LAND);
    await screen.findByText(/The purchase was sent and is still confirming/);
    expect(invalidated(stale)).toEqual(all(stale, true));
    expect(invalidated(untouched)).toEqual(all(untouched, false));
  });

  it("an approval still confirming sent no sale: it says so and reads nothing again", async () => {
    // The MANA approval's wait ran out: the sale was never sent, so onSubmitted never ran.
    vi.mocked(buyTradeWithMana).mockRejectedValueOnce(new TxPendingError(`0x${"a".repeat(64)}`, "The MANA approval"));
    await buy(LAND);
    await screen.findByText(APPROVAL_STILL_PENDING);
    expect(screen.queryByText(/The purchase was sent/)).toBeNull();
    expect(invalidated(stale)).toEqual(all(stale, false));
  });

  it("a sale sent and then lost track of reads the balance and that LAND again", async () => {
    vi.mocked(buyTradeWithMana).mockImplementationOnce(async (opts) => {
      opts.onSubmitted?.(SALE);
      throw new Error("the receipt read failed");
    });
    await buy(LAND);
    await screen.findByText(/The purchase was sent and is still confirming/);
    expect(invalidated(stale)).toEqual(all(stale, true));
  });

  it("a declined LAND trade spent nothing and reads nothing again", async () => {
    vi.mocked(buyTradeWithMana).mockRejectedValueOnce(new Error("User rejected the request."));
    await buy(LAND);
    await screen.findByText(/Nothing was bought/);
    expect(invalidated(stale)).toEqual(all(stale, false));
  });
});
