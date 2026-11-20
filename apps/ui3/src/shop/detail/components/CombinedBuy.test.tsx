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
import { batchAuthorization, relayedApproval } from "../../checkout/fakes";
import { CHAIN } from "@shop/model/chain";
import { createDemo, DemoShell } from "../../checkout/demo";
import { usdCents } from "@shop/model/money";
import { presentPayment, type MixedPayment } from "@shop/payment/present";
import { BOOK, storeMint } from "@shop/payment/fixtures";
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

  it("a credit that leaves more MANA to pay than was shown is refused, nothing is relayed, and the credit is released", async () => {
    // A 0.5 MANA credit on the 2.3 MANA mint: 1.8 MANA, above the 1.3 the buyer confirmed.
    const { demo } = setup(undefined, undefined, MANA / 2n);
    const manager = requireContracts(CHAIN.POLYGON).creditsManager!.address.toLowerCase();
    demo.chain.allowances.set(`${CHAIN.POLYGON}:${BOOK.buyer.toLowerCase()}:${manager}`, 10n ** 21n);
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(await screen.findByRole("alert", {}, { timeout: 5000 })).toHaveTextContent("Nothing was charged.");
    expect(demo.relay.posts).toHaveLength(0);
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(true);
  });

  it("a credit that leaves no MANA to pay is refused, never paid with the gap shown before, and released", async () => {
    // A 2.3 MANA credit on the 2.3 MANA mint: the terms the buyer confirmed no longer hold.
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
