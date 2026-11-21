import { useQueryClient, type QueryClient } from "@tanstack/react-query";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { SHOP_QUERY_KEYS } from "../../checkout/lib/after-purchase";
import { createDemo, DemoShell } from "../../checkout/demo";
import { APPROVAL_STILL_PENDING, TxPendingError } from "../../market/lib/tx";
import { executeLegacyOrder } from "../../market/purchase/executeOrder";
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

const balance = [SHOP_QUERY_KEYS.manaBalance, BOOK.buyer.toLowerCase(), 137];
const invalidated = (key: unknown[]) => pageQueries!.getQueryState(key)?.isInvalidated;
const SALE = `0x${"d".repeat(64)}`;

/** Opens the dialog over a page holding a MANA balance, and presses Buy. */
async function buy() {
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
  act(() => void pageQueries!.setQueryData(balance, 10n ** 21n));
  expect(invalidated(balance)).toBe(false);

  // Enabled once the wallet's network is read.
  await vi.waitFor(() => expect(screen.getByTestId("order-buy")).toBeEnabled());
  await act(async () => fireEvent.click(screen.getByTestId("order-buy")));
}

describe("LegacyOrderBuy", () => {
  it("a bought order has the page read its MANA balance again", async () => {
    await buy();
    await vi.waitFor(() => expect(invalidated(balance)).toBe(true));
  });

  it("a sale still confirming has the page read its MANA balance again too", async () => {
    // The sale went out (its hash reached onSubmitted) and then its wait ran out.
    vi.mocked(executeLegacyOrder).mockImplementationOnce(async (opts) => {
      opts.onSubmitted?.(SALE);
      throw new TxPendingError(SALE, "The purchase");
    });
    await buy();
    await screen.findByText(/The purchase was sent and is still confirming/);
    expect(invalidated(balance)).toBe(true);
  });

  it("an approval still confirming sent no sale: it says so and reads nothing again", async () => {
    // The MANA approval's wait ran out: the sale was never sent, so onSubmitted never ran.
    vi.mocked(executeLegacyOrder).mockRejectedValueOnce(new TxPendingError(`0x${"a".repeat(64)}`, "The MANA approval"));
    await buy();
    await screen.findByText(APPROVAL_STILL_PENDING);
    expect(screen.queryByText(/The purchase was sent/)).toBeNull();
    expect(invalidated(balance)).toBe(false);
  });

  it("a declined order spent nothing and reads nothing again", async () => {
    vi.mocked(executeLegacyOrder).mockRejectedValueOnce(new Error("User rejected the request."));
    await buy();
    await screen.findByText(/Nothing was bought/);
    expect(invalidated(balance)).toBe(false);
  });
});
