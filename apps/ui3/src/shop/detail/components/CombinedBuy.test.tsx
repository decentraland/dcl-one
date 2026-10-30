// The credits+MANA rail inside the web3 MANA option, run end to end on the checkout demo's fake node,
// chain and relay: the manager's MANA allowance approved first, ONE credit for the credits leg, the
// MANA gap checked against what was shown, and a declined signature releasing the credit.

import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { requireContracts } from "../../checkout/chain/contracts";
import { batchAuthorization } from "../../checkout/fakes";
import { CHAIN } from "../../model/chain";
import { createDemo, DemoShell } from "../../checkout/demo";
import { usdCents } from "../../model/money";
import { presentPayment, type MixedPayment } from "../../payment/present";
import { BOOK, storeMint } from "../../payment/fixtures";
import CombinedBuy from "./CombinedBuy";

function setup() {
  const demo = createDemo({
    session: "web3",
    handlers: {
      authorizeCredit: ({ body }) => {
        const cents = (body as { usdPriceCents: number }).usdPriceCents;
        const one = batchAuthorization([{ usdPriceCents: cents }]);
        return { ...one, credit: { ...one.credit, amount: "1000000000000000000", availableAmount: "1000000000000000000" }, maxCreditedValue: "1000000000000000000", usdCents: cents };
      },
    },
  });
  const offer = storeMint(demo.session);
  const view = presentPayment(offer, demo.session, { creditsBalance: usdCents(50), report: () => {} });
  const mixed = view.options.find((o) => o.method === "mana")!.mixed as MixedPayment;
  const line = { asset: offer.asset, listing: { acquisition: "store" as const }, display: { name: "Rainbow Hat" } };
  render(
    <DemoShell demo={demo} path="/shop/item/x/7">
      <CombinedBuy line={line} offer={offer} mixed={mixed} onClose={() => {}} portal={false} />
    </DemoShell>,
  );
  return { demo, mixed };
}

describe("buying with Credits and MANA", () => {
  it("approves the Credits manager's MANA, authorizes one credit for the credits leg, and relays one useCredits", async () => {
    const { demo, mixed } = setup();
    expect(mixed).toMatchObject({ credits: 5, creditsCents: 50 });
    expect(screen.getByTestId("combined-credits")).toHaveTextContent("5 Credits");
    expect(screen.getByTestId("combined-mana")).toHaveTextContent("1.3 MANA");
    await act(async () => fireEvent.click(screen.getByTestId("combined-confirm")));
    expect(demo.shop.log.some((r) => r.name === "authorizeCredit")).toBe(false);
    await act(async () => fireEvent.click(await screen.findByTestId("authorize-step-action")));
    expect(await screen.findByText(/Purchase complete/, {}, { timeout: 5000 })).toBeTruthy();
    const auth = demo.shop.log.filter((r) => r.name === "authorizeCredit");
    expect(auth).toHaveLength(1);
    expect(auth[0]!.body).toMatchObject({ usdPriceCents: 50, contractAddress: BOOK.collection, itemId: "7" });
    expect(demo.relay.posts).toHaveLength(2);
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(false);
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
});
