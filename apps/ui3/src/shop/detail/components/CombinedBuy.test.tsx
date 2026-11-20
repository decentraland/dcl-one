// The credits+MANA rail inside the web3 MANA option, run end to end on the checkout demo's fake node,
// chain and relay: the manager's MANA allowance approved first (exactly the MANA shown), ONE credit
// authorized at the whole price for the credits leg, the MANA gap checked against what was shown, and a
// declined signature releasing the credit.

import { useQueryClient, type QueryClient } from "@tanstack/react-query";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { decodeFunctionData, type Hex } from "viem";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { setSplitPayment } from "@shop/payment/present";

import { CREDITS_MANAGER_ABI, OFFCHAIN_META_TX_ABI } from "@shop/checkout/chain/abi";

import { requireContracts } from "../../checkout/chain/contracts";
import { batchAuthorization, creatorTrade, relayedApproval } from "../../checkout/fakes";
import { CHAIN } from "@shop/model/chain";
import { createDemo, DemoShell } from "../../checkout/demo";
import { usdCents } from "@shop/model/money";
import { presentPayment, type MixedPayment } from "@shop/payment/present";
import { BOOK, creatorOrder, storeMint } from "@shop/payment/fixtures";
import type { ManaWei } from "@shop/model/money";
import type { PurchaseOffer } from "@shop/model/purchase";
import { ASSET_TYPE } from "../../checkout/purchase/encoding";
import { cartLineId } from "../../cart/store";
import { SHOP_QUERY_KEYS } from "../../checkout/lib/after-purchase";
import CombinedBuy, { readSentSplit } from "./CombinedBuy";

afterEach(() => sessionStorage.clear());

/** The page's query client, as the dialog's own page holds it. */
let pageQueries: QueryClient | null = null;
function QueryProbe() {
  pageQueries = useQueryClient();
  return null;
}

const MANA = 10n ** 18n;

/** The useCredits arguments a relayed CreditsManager meta-transaction carries. */
function relayedUseCredits(body: unknown): { maxCreditedValue: bigint; maxUncreditedValue: bigint } {
  const data = (body as { transactionData: { params: [string, Hex] } }).transactionData.params[1];
  const meta = decodeFunctionData({ abi: OFFCHAIN_META_TX_ABI, data });
  return decodeFunctionData({ abi: CREDITS_MANAGER_ABI, data: meta.args[1] as Hex }).args[0] as { maxCreditedValue: bigint; maxUncreditedValue: bigint };
}

/** The node answers a credit for the Credits part (`creditsCents`), worth `value` (1 MANA by default). */
function setup(signedFor?: string, confirm?: "none", value = MANA) {
  const demo = createDemo({
    session: "web3",
    ...(confirm ? { confirm } : {}),
    handlers: {
      authorizeCredit: ({ body }) => {
        const cents = (body as { creditsCents: number }).creditsCents;
        const one = batchAuthorization([{ usdPriceCents: cents }]);
        const credit = { ...one.credit, amount: value.toString(), availableAmount: value.toString(), ...(signedFor ? { contract: signedFor } : {}) };
        return { ...one, credit, maxCreditedValue: value.toString(), usdCents: cents };
      },
    },
  });
  const offer = storeMint(demo.session);
  const view = presentPayment(offer, demo.session, { creditsBalance: usdCents(50), report: () => {} });
  const mixed = view.options.find((o) => o.method === "mana")!.mixed as MixedPayment;
  const line = { asset: offer.asset, listing: { acquisition: "store" as const }, display: { name: "Rainbow Hat" } };
  const shell = () => (
    <DemoShell demo={demo} path="/shop/item/x/7">
      <QueryProbe />
      <CombinedBuy line={line} offer={offer} mixed={mixed} onClose={() => {}} portal={false} />
    </DemoShell>
  );
  const shown = render(shell());
  return { demo, mixed, line, unmount: shown.unmount, again: () => render(shell()) };
}


// The split (Credits and MANA together) is off in this deployment (present's SPLIT_PAYMENT): these tests cover it as it
// runs once it is offered.
let undoSplit: () => void = () => {};
beforeEach(() => {
  undoSplit = setSplitPayment(true);
});
afterEach(() => undoSplit());

describe("buying with Credits and MANA", () => {
  it("approves the Credits manager exactly the MANA shown, authorizes one credit at the whole price for the credits leg, and relays one useCredits", async () => {
    const { demo, mixed } = setup();
    expect(mixed).toMatchObject({ priceCents: 115, credits: 5, creditsCents: 50 });
    expect(screen.getByTestId("combined-credits")).toHaveTextContent("5 Credits");
    expect(screen.getByTestId("combined-mana")).toHaveTextContent("1.3 MANA");
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(demo.shop.log.some((r) => r.name === "authorizeCredit")).toBe(false);
    await act(async () => fireEvent.click(await screen.findByTestId("authorize-step-action")));
    expect(await screen.findByText(/Purchase complete/, {}, { timeout: 5000 })).toBeTruthy();
    const auth = demo.shop.log.filter((r) => r.name === "authorizeCredit");
    expect(auth).toHaveLength(1);
    expect(auth[0]!.body).toMatchObject({ usdPriceCents: 115, creditsCents: 50, contractAddress: BOOK.collection, itemId: "7" });
    const manager = requireContracts(CHAIN.POLYGON).creditsManager!.address.toLowerCase();
    expect(demo.relay.posts).toHaveLength(2);
    expect(relayedApproval(demo.relay.posts[0])).toEqual({ owner: BOOK.buyer.toLowerCase(), spender: manager, amount: mixed.manaWei });
    // The 2.3 MANA mint: the 1 MANA credit and the 1.3 MANA gap, each its own cap.
    expect(relayedUseCredits(demo.relay.posts[1])).toMatchObject({ maxCreditedValue: MANA, maxUncreditedValue: (13n * MANA) / 10n });
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(false);
  });

  it("a finished purchase has the page read its MANA balance again", async () => {
    setup();
    const balance = [SHOP_QUERY_KEYS.manaBalance, BOOK.buyer.toLowerCase(), 137];
    act(() => void pageQueries!.setQueryData(balance, 10n ** 21n));
    expect(pageQueries!.getQueryState(balance)?.isInvalidated).toBe(false);
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    await act(async () => fireEvent.click(await screen.findByTestId("authorize-step-action")));
    expect(await screen.findByText(/Purchase complete/, {}, { timeout: 5000 })).toBeTruthy();
    expect(pageQueries!.getQueryState(balance)?.isInvalidated).toBe(true);
  });

  it("a credit below the node's share (which would leave more MANA than shown) is refused, nothing is relayed, and it is released", async () => {
    // A 0.5 MANA credit on the 2.3 MANA mint, where the share is 1 MANA: 1.8 MANA left, above the 1.3 confirmed.
    const { demo } = setup(undefined, undefined, MANA / 2n);
    const manager = requireContracts(CHAIN.POLYGON).creditsManager!.address.toLowerCase();
    demo.chain.allowances.set(`${CHAIN.POLYGON}:${BOOK.buyer.toLowerCase()}:${manager}`, 10n ** 21n);
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByRole("alert", {}, { timeout: 5000 })).toHaveTextContent("Nothing was charged.");
    expect(demo.relay.posts).toHaveLength(0);
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(true);
  });

  it("a listing paid in MANA: the credit is the share of its MANA, the gap the rest, each its own cap (the split vector, 91 wei)", async () => {
    // Quoted 30 cents and 91 MANA wei (the vector's 101 wei after its 10% coupon); 10 cents of Credits held.
    const demo = createDemo({
      session: "web3",
      resolvers: {
        trade: async () => creatorTrade({ received: [{ assetType: ASSET_TYPE.ERC20, contractAddress: BOOK.mana, amount: "91", extra: "0x", beneficiary: BOOK.creator }] }),
      },
      handlers: {
        authorizeCredit: ({ body }) => {
          const { usdPriceCents, creditsCents } = body as { usdPriceCents: number; creditsCents: number };
          const value = ((91n * BigInt(creditsCents)) / BigInt(usdPriceCents)).toString();
          const one = batchAuthorization([{ usdPriceCents: creditsCents }]);
          return { ...one, credit: { ...one.credit, amount: value, availableAmount: value }, maxCreditedValue: value, usdCents: creditsCents };
        },
      },
    });
    const listed = creatorOrder(demo.session);
    const offer: PurchaseOffer = {
      ...listed,
      quotes: [
        { method: "credits", quoted: usdCents(30) },
        { method: "mana", amount: 91n as ManaWei },
      ],
      splittable: true,
    };
    const mixed = presentPayment(offer, demo.session, { creditsBalance: usdCents(10), report: () => {} }).options.find((o) => o.method === "mana")!.mixed!;
    expect(mixed).toEqual({ priceCents: 30, creditsCents: 10, credits: 1, manaWei: 61n });
    const line = { asset: offer.asset, listing: { tradeId: "trade-creator-1" }, display: { name: "Rainbow Hat" } };
    render(
      <DemoShell demo={demo} path="/shop/item/x/7">
        <CombinedBuy line={line} offer={offer} mixed={mixed} onClose={() => {}} portal={false} />
      </DemoShell>,
    );
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    await act(async () => fireEvent.click(await screen.findByTestId("authorize-step-action")));
    expect(await screen.findByText(/Purchase complete/, {}, { timeout: 5000 })).toBeTruthy();
    const auth = demo.shop.log.filter((r) => r.name === "authorizeCredit");
    expect(auth.map((r) => r.body)).toEqual([expect.objectContaining({ usdPriceCents: 30, creditsCents: 10, tradeId: "trade-creator-1" })]);
    const manager = requireContracts(CHAIN.POLYGON).creditsManager!.address.toLowerCase();
    expect(relayedApproval(demo.relay.posts[0])).toEqual({ owner: BOOK.buyer.toLowerCase(), spender: manager, amount: 61n });
    expect(relayedUseCredits(demo.relay.posts[1])).toMatchObject({ maxCreditedValue: 30n, maxUncreditedValue: 61n });
  });

  it("a USD-pegged listing is refused before any credit is authorized: its MANA is only fixed at settlement", async () => {
    let resolved = 0;
    const demo = createDemo({
      session: "web3",
      resolvers: {
        trade: async () => {
          resolved++;
          return creatorTrade();
        },
      },
    });
    const manager = requireContracts(CHAIN.POLYGON).creditsManager!.address.toLowerCase();
    demo.chain.allowances.set(`${CHAIN.POLYGON}:${BOOK.buyer.toLowerCase()}:${manager}`, 10n ** 21n);
    const offer = creatorOrder(demo.session);
    // Never offered for such a line (present.ts); the dialog refuses it all the same.
    const mixed: MixedPayment = { priceCents: usdCents(100), creditsCents: usdCents(50), credits: 5, manaWei: (MANA / 2n) as MixedPayment["manaWei"] };
    const line = { asset: offer.asset, listing: { tradeId: "trade-creator-1" }, display: { name: "Rainbow Hat" } };
    render(
      <DemoShell demo={demo} path="/shop/item/x/7">
        <CombinedBuy line={line} offer={offer} mixed={mixed} onClose={() => {}} portal={false} />
      </DemoShell>,
    );
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByRole("alert", {}, { timeout: 5000 })).toHaveTextContent("Nothing was charged.");
    expect(resolved).toBeGreaterThan(0);
    expect(demo.shop.log.some((r) => r.name === "authorizeCredit")).toBe(false);
    expect(demo.relay.posts).toHaveLength(0);
  });

  it("a credit above the node's share (here the whole price, no MANA left) is refused, never paid with the gap shown before, and released", async () => {
    // A 2.3 MANA credit on the 2.3 MANA mint, where the share is 1 MANA: the terms the buyer confirmed no longer hold.
    const { demo } = setup(undefined, undefined, (23n * MANA) / 10n);
    const manager = requireContracts(CHAIN.POLYGON).creditsManager!.address.toLowerCase();
    demo.chain.allowances.set(`${CHAIN.POLYGON}:${BOOK.buyer.toLowerCase()}:${manager}`, 10n ** 21n);
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByRole("alert", {}, { timeout: 5000 })).toHaveTextContent("We couldn't complete the purchase. Nothing was charged.");
    expect(demo.relay.posts).toHaveLength(0);
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(true);
  });

  it("a declined signature releases the credit and says nothing was charged", async () => {
    const { demo } = setup();
    const manager = requireContracts(CHAIN.POLYGON).creditsManager!.address.toLowerCase();
    demo.chain.allowances.set(`${CHAIN.POLYGON}:${BOOK.buyer.toLowerCase()}:${manager}`, 10n ** 21n);
    demo.wallet.next = "reject";
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByText("You cancelled the request in your wallet. Nothing was charged.", {}, { timeout: 5000 })).toBeTruthy();
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(true);
  });

  it("refuses a credit signed for a manager the Shop does not know, before approving or relaying anything", async () => {
    const stranger = "0x000000000000000000000000000000000000dead";
    const { demo } = setup(stranger);
    const manager = requireContracts(CHAIN.POLYGON).creditsManager!.address.toLowerCase();
    demo.chain.allowances.set(`${CHAIN.POLYGON}:${BOOK.buyer.toLowerCase()}:${manager}`, 10n ** 21n);
    const before = new Map(demo.chain.allowances);
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByRole("alert", {}, { timeout: 5000 })).toBeTruthy();
    expect(demo.relay.posts).toHaveLength(0);
    expect(demo.chain.allowances).toEqual(before);
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(true);
  });

  it("a split left confirming is recorded for this account and item, so a dialog opened again offers no second one", async () => {
    const { line, unmount, again } = setup(undefined, "none");
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    await act(async () => fireEvent.click(await screen.findByTestId("authorize-step-action")));
    expect(await screen.findByText(/still confirming/, {}, { timeout: 8000 })).toBeTruthy();
    expect(readSentSplit(BOOK.buyer, cartLineId(line.asset, line.listing))).not.toBeNull();
    unmount();
    again();
    expect(await screen.findByText(/still confirming/)).toBeTruthy();
    expect(screen.queryByTestId("combined-confirm")).toBeNull();
    expect(screen.getByTestId("combined-check-again")).toBeEnabled();
  });
});

describe("a split whose outcome is not known", () => {
  const managerKey = () => `${CHAIN.POLYGON}:${BOOK.buyer.toLowerCase()}:${requireContracts(CHAIN.POLYGON).creditsManager!.address.toLowerCase()}`;
  const authorizations = (demo: ReturnType<typeof createDemo>) => demo.shop.log.filter((r) => r.name === "authorizeCredit").length;

  it("a sale the relay may have sent (its answer lost) is held confirming, kept on reopen, and nothing is authorized or sent again", async () => {
    const { demo, line, unmount, again } = setup();
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    demo.relay.state.mode = "fail-after-send";
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByText(/still confirming/, {}, { timeout: 5000 })).toBeTruthy();
    expect(demo.relay.posts).toHaveLength(1);
    expect(authorizations(demo)).toBe(1);
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(false);
    const held = readSentSplit(BOOK.buyer, cartLineId(line.asset, line.listing));
    expect(held).toMatchObject({ txHash: null, chainId: CHAIN.POLYGON, creditId: expect.any(String), creditExpiresAt: expect.any(Number) });
    unmount();
    again();
    expect(await screen.findByText(/still confirming/)).toBeTruthy();
    expect(screen.queryByTestId("combined-confirm")).toBeNull();
    expect(screen.getByTestId("combined-check-again")).toBeDisabled();
    expect(demo.relay.posts).toHaveLength(1);
    expect(authorizations(demo)).toBe(1);
  });

  it("control: a sale the relay marked as not sent is not held; declined in the wallet, its credit is released", async () => {
    const { demo, line } = setup();
    demo.chain.allowances.set(managerKey(), 10n ** 21n);
    demo.relay.state.mode = "refuse";
    demo.wallet.next = "reject";
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByText("You cancelled the request in your wallet. Nothing was charged.", {}, { timeout: 5000 })).toBeTruthy();
    expect(readSentSplit(BOOK.buyer, cartLineId(line.asset, line.listing))).toBeNull();
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(true);
    expect(screen.getByTestId("combined-confirm")).toBeEnabled();
  });

  it("an approval whose answer was lost stays pending through Approve and its retry: no second grant, and no credit or sale until it lands", async () => {
    const { demo } = setup();
    demo.relay.state.mode = "fail-after-send";
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    await act(async () => fireEvent.click(await screen.findByTestId("authorize-step-action")));
    const pending = /Your approval was sent and is still confirming/;
    expect(await screen.findByText(pending, {}, { timeout: 5000 })).toBeTruthy();
    expect(demo.relay.posts).toHaveLength(1);
    // It has not landed yet (the fake relay applied it; take it back): a retry reads the allowance and grants nothing.
    demo.chain.allowances.delete(managerKey());
    await act(async () => fireEvent.click(screen.getByTestId("authorize-step-action")));
    expect(await screen.findByText(pending, {}, { timeout: 5000 })).toBeTruthy();
    expect(demo.relay.posts).toHaveLength(1);
    expect(demo.wallet.requests.some((r) => r.method === "eth_sendTransaction")).toBe(false);
    expect(authorizations(demo)).toBe(0);
    // It lands: the retry finds it, grants nothing more, and the purchase goes on.
    demo.chain.allowances.set(managerKey(), 13n * 10n ** 17n);
    demo.relay.state.mode = "ok";
    await act(async () => fireEvent.click(screen.getByTestId("authorize-step-action")));
    expect(await screen.findByText(/Purchase complete/, {}, { timeout: 5000 })).toBeTruthy();
    expect(demo.relay.posts.map(relayedApproval).filter((a) => a !== null)).toHaveLength(1);
    expect(authorizations(demo)).toBe(1);
  });
});

describe("a held split with no transaction to read", () => {
  const CREDIT = `0x${"c1".repeat(32)}`;
  function heldSplit(status: { value: "PENDING" | "SETTLED" | "EXPIRED" | null }, creditExpiresAt: number) {
    const demo = createDemo({
      session: "web3",
      handlers: {
        userPurchases: () => ({
          // As the node lists it: the credit's salt and the line's position.
          purchases: status.value ? [{ id: `${CREDIT}:0`, status: status.value, usdCents: 50, createdAt: 1, txHash: status.value === "SETTLED" ? `0x${"ab".repeat(32)}` : null }] : [],
          total: status.value ? 1 : 0,
        }),
      },
    });
    const offer = storeMint(demo.session);
    const mixed = presentPayment(offer, demo.session, { creditsBalance: usdCents(50), report: () => {} }).options.find((o) => o.method === "mana")!.mixed as MixedPayment;
    const line = { asset: offer.asset, listing: { acquisition: "store" as const }, display: { name: "Rainbow Hat" } };
    // Sent sixteen minutes ago, past the hold; the relay lost its answer, so no hash.
    sessionStorage.setItem(
      `dcl-shop-buynow-split:v1:${BOOK.buyer.toLowerCase()}:${cartLineId(line.asset, line.listing)}`,
      JSON.stringify({ at: Date.now() - 16 * 60 * 1000, txHash: null, chainId: CHAIN.POLYGON, creditId: CREDIT, creditExpiresAt }),
    );
    render(
      <DemoShell demo={demo} path="/shop/item/x/7">
        <CombinedBuy line={line} offer={offer} mixed={mixed} onClose={() => {}} portal={false} />
      </DemoShell>,
    );
    return demo;
  }
  const nowSeconds = () => Math.floor(Date.now() / 1000);

  it("is not let go by time while its credit can still be spent", async () => {
    heldSplit({ value: "EXPIRED" }, nowSeconds() + 300);
    expect(await screen.findByText(/still confirming/)).toBeTruthy();
    expect(screen.getByTestId("combined-check-again")).toBeDisabled();
  });

  it("once its credit expired, the node's record decides: still reserved keeps it held, released lets the item go", async () => {
    const status: { value: "PENDING" | "SETTLED" | "EXPIRED" | null } = { value: "PENDING" };
    const demo = heldSplit(status, nowSeconds() - 1);
    const check = await screen.findByTestId("combined-check-again");
    expect(check).toBeEnabled();
    await act(async () => fireEvent.click(check));
    expect(screen.getByText(/still confirming/)).toBeTruthy();
    status.value = null;
    await act(async () => fireEvent.click(screen.getByTestId("combined-check-again")));
    expect(screen.getByText(/still confirming/)).toBeTruthy();
    status.value = "EXPIRED";
    await act(async () => fireEvent.click(screen.getByTestId("combined-check-again")));
    expect(await screen.findByTestId("combined-confirm")).toBeEnabled();
    expect(demo.shop.log.filter((r) => r.name === "userPurchases")).toHaveLength(3);
  });

  it("a credit the node says was spent completes the purchase, never offering it again", async () => {
    heldSplit({ value: "SETTLED" }, nowSeconds() - 1);
    await act(async () => fireEvent.click(await screen.findByTestId("combined-check-again")));
    expect(await screen.findByText(/Purchase complete/)).toBeTruthy();
    expect(screen.queryByTestId("combined-confirm")).toBeNull();
  });
});
