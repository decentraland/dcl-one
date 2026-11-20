import { useQueryClient, type QueryClient } from "@tanstack/react-query";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { SHOP_QUERY_KEYS } from "../../checkout/lib/after-purchase";
import { createDemo, DemoShell } from "../../checkout/demo";
import { BOOK } from "@shop/payment/fixtures";
import LegacyOrderBuy from "./LegacyOrderBuy";

// The wallet's side of the order is not under test here: it settles at once with one transaction.
vi.mock("../../market/purchase/executeOrder", () => ({
  executeLegacyOrder: vi.fn(async () => `0x${"f".repeat(64)}`),
}));

/** The page's query client, as the dialog's own page holds it. */
let pageQueries: QueryClient | null = null;
function QueryProbe() {
  pageQueries = useQueryClient();
  return null;
}

describe("LegacyOrderBuy", () => {
  it("a bought order has the page read its MANA balance again", async () => {
    const demo = createDemo({ session: "web3" });
    render(
      <DemoShell demo={demo} path={`/shop/token/${BOOK.collection}/42`}>
        <QueryProbe />
        <LegacyOrderBuy
          name="Vintage Jacket"
          contractAddress={BOOK.collection}
          tokenId="42"
          order={{ id: "order-1", marketplaceAddress: `0x${"1".repeat(40)}`, price: "1000000000000000000", chainId: 137 }}
          quoted={null}
          onClose={() => {}}
          portal={false}
        />
      </DemoShell>,
    );
    const balance = [SHOP_QUERY_KEYS.manaBalance, BOOK.buyer.toLowerCase(), 137];
    act(() => void pageQueries!.setQueryData(balance, 10n ** 21n));
    expect(pageQueries!.getQueryState(balance)?.isInvalidated).toBe(false);

    // Enabled once the wallet's network is read.
    await vi.waitFor(() => expect(screen.getByTestId("order-buy")).toBeEnabled());
    await act(async () => fireEvent.click(screen.getByTestId("order-buy")));
    await vi.waitFor(() => expect(pageQueries!.getQueryState(balance)?.isInvalidated).toBe(true));
  });
});
