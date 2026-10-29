import { act, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { createDemo, DemoShell, type DemoScenario } from "../checkout/demo";
import type { PurchaseRecord } from "../credits/credits";
import { BOOK } from "../payment/fixtures";
import { buildActivityFeed, filterActivity, type SaleRecord } from "./activity";
import ActivityPage from "./ActivityPage";
import BuyStatusPage from "./BuyStatusPage";
import { foldOrderLines, groupPurchases, purchaseOrderPill } from "./purchases";
import { saveReceipt, type SuccessNavState } from "./receipt";
import SuccessPage from "./SuccessPage";

const rec = (over: Partial<PurchaseRecord>): PurchaseRecord => ({
  id: "r",
  tradeId: null,
  contractAddress: BOOK.collection,
  itemId: "7",
  registeredName: null,
  usdCents: 120,
  credits: 12,
  status: "SETTLED",
  createdAt: 1_000_000,
  manaSettledWei: null,
  txHash: null,
  submittedTxHash: null,
  ...over,
});

const sale = (over: Partial<SaleRecord>): SaleRecord => ({
  id: "s",
  buyer: BOOK.buyer,
  seller: BOOK.reseller,
  contractAddress: BOOK.collection,
  tokenId: "42",
  itemId: null,
  manaWei: "2300000000000000000",
  createdAt: 2_000_000,
  txHash: "0xsale",
  category: "wearable",
  chainId: 137,
  ...over,
});

describe("purchase history folded into orders", () => {
  it("one card per settlement transaction; hashless lines fold by time and status", () => {
    const orders = groupPurchases([
      rec({ id: "a", txHash: "0xT", createdAt: 10 }),
      rec({ id: "b", txHash: "0xT", createdAt: 11 }),
      rec({ id: "c", status: "PENDING", createdAt: 100_000 }),
      rec({ id: "d", status: "PENDING", createdAt: 105_000 }),
      rec({ id: "e", status: "EXPIRED", createdAt: 108_000 }),
    ]);
    expect(orders.map((o) => [o.id, o.status, o.lines.length])).toEqual([
      ["e", "EXPIRED", 1],
      ["c", "PENDING", 2],
      ["0xT", "SETTLED", 2],
    ]);
    expect(purchaseOrderPill("EXPIRED")).toBe("FAILED");
    expect(foldOrderLines(orders[2]!.lines)).toEqual([expect.objectContaining({ quantity: 2, credits: 24 })]);
  });

  it("the feed keeps an EXPIRED purchase only when it was submitted, drops dead pack orders, and never double-counts a credits settlement", () => {
    const feed = buildActivityFeed({
      purchases: [rec({ id: "shown", status: "EXPIRED", submittedTxHash: "0xs", createdAt: 5 }), rec({ id: "noise", status: "EXPIRED", createdAt: 6 }), rec({ id: "ok", txHash: "0xdup", createdAt: 7 })],
      sales: [sale({ id: "sold", createdAt: 9 })],
      creditOrders: [
        { id: "paid", credits: 100, usdCents: 1199, status: "credited", createdAt: 8 },
        { id: "dead", credits: 40, usdCents: 599, status: "abandoned", createdAt: 3 },
        { id: "open", credits: 40, usdCents: 599, status: "initiated", createdAt: 4 },
      ],
      manaPurchases: [sale({ id: "via-credits", txHash: "0xDUP", createdAt: 7 }), sale({ id: "mana", txHash: "0xmana", createdAt: 2 })],
    });
    expect(feed.map((e) => e.id)).toEqual(["sale:sold", "credit:paid", "purchase:0xdup", "purchase:shown", "credit:open", "mana-purchase:mana"]);
    expect(filterActivity(feed, "sales").map((e) => e.kind)).toEqual(["sale"]);
    expect(filterActivity(feed, "purchases").some((e) => e.kind === "sale")).toBe(false);
  });
});

function renderAt(node: React.ReactNode, s: Partial<DemoScenario> & { path: string; pattern?: string }) {
  const demo = createDemo({ session: "web3", ...s });
  render(
    <DemoShell demo={demo} path={s.path} routes={{ "/shop/items": <p>ITEMS PAGE</p> }}>
      {node}
    </DemoShell>,
  );
  return demo;
}

describe("the activity page", () => {
  it("signed out: asks to sign in", () => {
    renderAt(<ActivityPage />, { session: "signedOut", path: "/shop/activity" });
    expect(screen.getByTestId("activity-signed-out").textContent).toContain("Sign in to see your activity");
  });

  it("empty: says nothing happened yet", async () => {
    renderAt(<ActivityPage />, { path: "/shop/activity", handlers: { userPurchases: () => ({ purchases: [], total: 0 }), userCreditOrders: () => ({ orders: [] }), sales: () => ({ data: [], total: 0 }) } });
    expect(await screen.findByTestId("activity-empty-all")).toBeTruthy();
  });

  it("error: says so and offers a retry", async () => {
    renderAt(<ActivityPage />, { path: "/shop/activity", handlers: { userPurchases: () => ({ status: 500, body: { error: "down" } }), sales: () => ({ status: 500, body: { error: "down" } }) } });
    expect(await screen.findByTestId("activity-error")).toBeTruthy();
  });

  it("orders show Credits, a failed one says the Credits came back; sales show the exact MANA received and no credit figure", async () => {
    renderAt(<ActivityPage />, {
      path: "/shop/activity",
      handlers: {
        userPurchases: () => ({ purchases: [{ id: "x", status: "EXPIRED", usdCents: 115, credits: 12, createdAt: 5, submittedTxHash: "0xs", contractAddress: BOOK.collection, itemId: "7" }], total: 1 }),
        userCreditOrders: () => ({ orders: [{ id: "o", credits: 100, usdCents: 1199, status: "initiated", createdAt: 4 }] }),
        sales: ({ query }) => (query.get("seller") ? { data: [{ id: "s1", buyer: BOOK.buyer, seller: BOOK.reseller, contractAddress: BOOK.collection, tokenId: "42", itemId: null, price: "2300000000000000000", timestamp: 9, txHash: "0xs1", chainId: 137 }], total: 1 } : { data: [], total: 0 }),
        items: () => ({ data: [{ name: "Rainbow Hat", thumbnail: "" }] }),
        nfts: () => ({ data: [{ nft: { name: "Vintage Jacket", image: "" } }] }),
      },
    });
    const order = await screen.findByTestId("purchase-order");
    expect(order.textContent).toContain("Failed");
    expect(order.textContent).toContain("The Credits it reserved are back in your balance.");
    const soldCard = screen.getByTestId("activity-sale");
    expect(soldCard.textContent).toMatch(/\+\s*2\.3\s*MANA/);
    expect(soldCard.textContent).not.toMatch(/credit/i);
    await within(soldCard).findByText("Vintage Jacket");
    expect(within(screen.getByTestId("credit-order")).getByRole("button", { name: "Continue" })).toBeTruthy();
  });
});

const RECEIPT: SuccessNavState = {
  items: [
    { name: "Rainbow Hat", quantity: 2, method: "credits" },
    { name: "Vintage Jacket", quantity: 1, method: "mana" },
  ],
  txs: [
    { hash: `0x${"a".repeat(64)}`, chainId: 137, method: "credits" },
    { hash: `0x${"b".repeat(64)}`, chainId: 137, method: "mana" },
  ],
  settled: true,
  creditsAdded: 100,
};

describe("the success page (receipt)", () => {
  it("no receipt (a stale link): back to the catalogue", async () => {
    sessionStorage.clear();
    renderAt(<SuccessPage />, { path: "/shop/success", handlers: {} });
    expect(await screen.findByText("ITEMS PAGE")).toBeTruthy();
  });

  it("settled, from the stored receipt after a reload: what was bought, how it was paid, each transaction linked for a self-custody wallet", async () => {
    saveReceipt(RECEIPT);
    renderAt(<SuccessPage />, { path: "/shop/success" });
    await act(async () => {});
    const done = screen.getByTestId("success-done");
    expect(done.textContent).toContain("Rainbow Hat × 2");
    expect(done.textContent).toContain("Paid with Credits");
    expect(done.textContent).toContain("Paid with MANA");
    expect(screen.getByTestId("success-credits").textContent).toContain("100 Credits added");
    expect(within(done).getAllByRole("link", { name: /Receipt|View receipt/ }).map((a) => a.getAttribute("href"))).toEqual([
      `https://polygonscan.com/tx/0x${"a".repeat(64)}`,
      `https://polygonscan.com/tx/0x${"b".repeat(64)}`,
    ]);
  });

  it("a managed wallet sees no explorer jargon", () => {
    saveReceipt(RECEIPT);
    renderAt(<SuccessPage />, { session: "web2", path: "/shop/success" });
    expect(screen.queryByRole("link", { name: /receipt/i })).toBeNull();
  });

  it("unsettled: follows the transactions; a late receipt completes it, a revert says nothing was delivered", async () => {
    saveReceipt({ ...RECEIPT, settled: false });
    const demo = renderAt(<SuccessPage />, { path: "/shop/success" });
    expect(screen.getByTestId("success-pending")).toBeTruthy();
    demo.chain.receipts.set(`0x${"a".repeat(64)}`, "success");
    demo.chain.receipts.set(`0x${"b".repeat(64)}`, "success");
    await screen.findByTestId("success-done");
  });

  it("unsettled and reverted: failed", async () => {
    saveReceipt({ ...RECEIPT, settled: false });
    const demo = renderAt(<SuccessPage />, { path: "/shop/success" });
    demo.chain.receipts.set(`0x${"a".repeat(64)}`, "reverted");
    demo.chain.receipts.set(`0x${"b".repeat(64)}`, "success");
    await screen.findByTestId("success-failed");
  });
});

describe("the buy status page", () => {
  function renderStatus(path: string, s: Partial<DemoScenario> = {}) {
    const demo = createDemo({ session: "web3", ...s });
    const pattern = path.includes("/token/") ? "/shop/token/:contractAddress/:tokenId/buy/status" : "/shop/item/:contractAddress/:itemId/buy/status";
    render(
      <DemoShell demo={demo} path={path} pattern={pattern}>
        <BuyStatusPage />
      </DemoShell>,
    );
    return demo;
  }

  it("with a transaction: pending, then complete on its receipt", async () => {
    const hash = `0x${"c".repeat(64)}`;
    const demo = renderStatus(`/shop/token/${BOOK.collection}/42/buy/status?txHash=${hash}&chainId=137`, { handlers: { nfts: () => ({ data: [{ nft: { name: "Vintage Jacket" } }] }) } });
    expect(screen.getByTestId("buy-status").getAttribute("data-status")).toBe("pending");
    demo.chain.receipts.set(hash, "success");
    await waitFor(() => expect(screen.getByTestId("buy-status").getAttribute("data-status")).toBe("complete"));
    expect(screen.getByRole("link", { name: "View transaction" }).getAttribute("href")).toBe(`https://polygonscan.com/tx/${hash}`);
  });

  it("with a reverted transaction: failed, nothing delivered", async () => {
    const hash = `0x${"d".repeat(64)}`;
    const demo = renderStatus(`/shop/item/${BOOK.collection}/7/buy/status?txHash=${hash}&chainId=137`, { handlers: { items: () => ({ data: [] }) } });
    demo.chain.receipts.set(hash, "reverted");
    await waitFor(() => expect(screen.getByTestId("buy-status").getAttribute("data-status")).toBe("failed"));
  });

  it("without one: the account's own purchase record decides, or it says none was found", async () => {
    renderStatus(`/shop/item/${BOOK.collection}/7/buy/status`, {
      handlers: { userPurchases: () => ({ purchases: [{ id: "p", status: "PENDING", usdCents: 115, createdAt: 1, contractAddress: BOOK.collection, itemId: "7" }], total: 1 }), items: () => ({ data: [] }) },
    });
    await waitFor(() => expect(screen.getByTestId("buy-status").getAttribute("data-status")).toBe("pending"));
  });
});
