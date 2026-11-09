import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useLocation } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import { createDemo, DemoShell, type DemoScenario } from "../checkout/demo";
import type { FakeHandlers } from "../checkout/fakes";
import { BOOK } from "../payment/fixtures";
import type { ShopWalletHost } from "../wallet/session";
import CreditsPage from "./CreditsPage";
import { bindBuyNowTopUp, stashBuyNowTopUp, stashTopUpReturn, takeTopUpReturn } from "./returns";

afterEach(() => sessionStorage.clear());

/** The item page the credits page sends a Buy now buyer back to, showing the state it was handed. */
function ItemState() {
  const { state } = useLocation();
  return <p data-testid="item-state">{JSON.stringify(state)}</p>;
}

function renderCredits(s: Partial<DemoScenario> & { path?: string; handlers?: FakeHandlers } = {}) {
  const demo = createDemo({ session: "web3", ...s });
  const openExternal = vi.fn();
  const onSignIn = vi.fn();
  demo.services.openExternal = openExternal;
  demo.host.onSignIn = onSignIn;
  const view = render(
    <DemoShell demo={demo} path={s.path ?? "/shop/credits"} routes={{ "/shop/cart": <p>CART PAGE</p>, "/shop/items": <p>ITEMS PAGE</p>, "/shop/item/:contract/:item": <ItemState /> }}>
      <CreditsPage />
    </DemoShell>,
  );
  return { demo, openExternal, onSignIn, ...view };
}

function renderBeforeSession(s: Partial<DemoScenario> & { path: string; handlers?: FakeHandlers }) {
  const demo = createDemo({ session: "web3", ...s });
  const shell = (host: ShopWalletHost) => (
    <DemoShell demo={{ ...demo, host }} path={s.path}>
      <CreditsPage />
    </DemoShell>
  );
  const view = render(shell({ ...demo.host, identity: null, walletKind: null, resolving: true }));
  return { demo, resolve: () => view.rerender(shell(demo.host)) };
}

describe("the credits page", () => {
  it("shows the server's packs in order; the balance and what is on hold live in the Shop's bars, as upstream", async () => {
    renderCredits({ balanceCents: 1234, held: { cents: 90, credits: 9, releasesAtSeconds: null, purchases: [] } });
    const packs = await screen.findAllByTestId("pack");
    expect(screen.queryByTestId("credits-balance")).toBeNull();
    expect(screen.queryByTestId("held-credits")).toBeNull();
    expect(packs.map((p) => p.getAttribute("aria-label"))).toEqual([
      "40 Credits for $5.99",
      expect.stringMatching(/^100 Credits for \$11\.99/),
      expect.stringMatching(/^260 Credits for \$29\.99/),
      expect.stringMatching(/^540 Credits for \$59\.99/),
    ]);
  });

  it("signed out: the packs are visible, choosing one asks to sign in", async () => {
    const { onSignIn } = renderCredits({ session: "signedOut" });
    fireEvent.click((await screen.findAllByTestId("pack"))[1]!);
    expect(onSignIn).toHaveBeenCalled();
  });

  it("choosing a pack sends the buyer to the hosted checkout", async () => {
    const { openExternal, demo } = renderCredits();
    const pack = (await screen.findAllByTestId("pack"))[1]!;
    await act(async () => fireEvent.click(pack));
    await waitFor(() => expect(openExternal).toHaveBeenCalledWith("https://checkout.stripe.com/c/pay/demo"));
    expect(demo.shop.log.find((r) => r.name === "createPackOrder")!.body).toMatchObject({ packId: "pack_10", source: "website" });
  });

  it("Stripe return, credited: the grant is shown", async () => {
    renderCredits({ path: "/shop/credits?order=order-1", handlers: { packOrder: () => ({ status: "credited", creditsGranted: 100, newBalance: 200 }) } });
    const success = (await screen.findByTestId("credits-success")).textContent;
    expect(success).toContain("Your purchase was successful!");
    expect(success).toContain("100 Credits Added to your account");
  });

  it("Stripe return, credited, for a cart that ran short: back to the cart to resume it", async () => {
    stashTopUpReturn("/shop/cart");
    renderCredits({ path: "/shop/credits?order=order-1", handlers: { packOrder: () => ({ status: "credited", creditsGranted: 100 }) } });
    await screen.findByText("CART PAGE");
  });

  it("Stripe return, credited, for Buy now's top-up: back to the item, with the order it credited", async () => {
    const attempt = stashBuyNowTopUp({ path: "/shop/item/0xc0/7", account: BOOK.buyer, lineId: "line-7", credits: 12 });
    bindBuyNowTopUp(attempt, "order-1");
    renderCredits({ path: "/shop/credits?order=order-1", handlers: { packOrder: () => ({ status: "credited", creditsGranted: 100 }) } });
    expect(await screen.findByTestId("item-state")).toHaveTextContent('"topUpOrder":"order-1"');
  });

  it("Stripe return, credited, for another order: Buy now's abandoned top-up is not finished, and is dropped", async () => {
    const attempt = stashBuyNowTopUp({ path: "/shop/item/0xc0/7", account: BOOK.buyer, lineId: "line-7", credits: 12 });
    bindBuyNowTopUp(attempt, "order-1");
    renderCredits({ path: "/shop/credits?order=order-2", handlers: { packOrder: () => ({ status: "credited", creditsGranted: 100 }) } });
    expect(await screen.findByTestId("credits-success")).toBeTruthy();
    expect(screen.queryByTestId("item-state")).toBeNull();
    expect(sessionStorage.getItem("dcl-shop-buynow-return:v1")).toBeNull();
  });

  it("a pack bought here, or an older order resumed, finishes nothing an earlier purchase left behind", async () => {
    stashBuyNowTopUp({ path: "/shop/item/0xc0/7", account: BOOK.buyer, lineId: "line-7", credits: 12 });
    const { openExternal } = renderCredits();
    await act(async () => fireEvent.click((await screen.findAllByTestId("pack"))[0]!));
    await waitFor(() => expect(openExternal).toHaveBeenCalled());
    expect(sessionStorage.getItem("dcl-shop-buynow-return:v1")).toBeNull();
    expect(takeTopUpReturn()).toBeNull();
  });

  it("paid but not landed yet: 'on the way', never an error", async () => {
    renderCredits({ path: "/shop/credits?order=order-1", handlers: { packOrder: () => ({ status: "pending" }) } });
    expect((await screen.findByTestId("credits-pending")).textContent).toContain("Your Credits are on the way");
  });

  it("a failed grant says so; an unpaid order goes back to the packs with the canceled note", async () => {
    renderCredits({ path: "/shop/credits?order=order-1", handlers: { packOrder: () => ({ status: "failed", error: "declined" }) } });
    expect((await screen.findByTestId("credits-error")).textContent).toContain("We couldn't add your Credits");
  });

  it("Stripe return on a full page load: waits for the session, then checks a signed-in buyer's order", async () => {
    const { demo, resolve } = renderBeforeSession({ path: "/shop/credits?order=order-1", handlers: { packOrder: () => ({ status: "credited", creditsGranted: 100 }) } });
    expect(screen.queryByTestId("credits-error")).toBeNull();
    resolve();
    expect((await screen.findByTestId("credits-success")).textContent).toContain("100 Credits Added to your account");
    expect(screen.queryByText(/you're signed out/)).toBeNull();
    expect(demo.shop.log.some((r) => r.name === "packOrder" && r.params.orderId === "order-1")).toBe(true);
  });

  it("Stripe return on a full page load, truly signed out: says so once the session is read, and checks nothing", async () => {
    const { demo, resolve } = renderBeforeSession({ session: "signedOut", path: "/shop/credits?order=order-1" });
    expect(screen.queryByTestId("credits-error")).toBeNull();
    resolve();
    expect((await screen.findByTestId("credits-error")).textContent).toContain("Your payment went through, but you're signed out");
    expect(demo.shop.log.some((r) => r.name === "packOrder")).toBe(false);
  });

  it("canceled at Stripe: the order is retired and the packs come back with a note", async () => {
    const { demo } = renderCredits({ path: "/shop/credits?order=order-9&canceled=1", handlers: { cancelPackOrder: () => ({ ok: true }) } });
    await screen.findByText(/^Payment canceled/);
    await waitFor(() => expect(demo.shop.log.some((r) => r.name === "cancelPackOrder" && r.params.orderId === "order-9")).toBe(true));
  });

  it("an unfinished pack checkout can be continued from the recent purchases", async () => {
    const { openExternal } = renderCredits({
      handlers: {
        userCreditOrders: () => ({ orders: [{ id: "o-1", credits: 100, usdCents: 1199, status: "initiated", createdAt: Date.now() }], total: 1 }),
        resumePackOrder: () => ({ url: "https://checkout.stripe.com/c/pay/again" }),
      },
    });
    const order = await screen.findByTestId("credit-order");
    expect(order.textContent).toContain("Not paid yet");
    stashTopUpReturn("/shop/cart");
    await act(async () => fireEvent.click(within(order).getByTestId("resume-order")));
    await waitFor(() => expect(openExternal).toHaveBeenCalledWith("https://checkout.stripe.com/c/pay/again"));
    // An older order resumed is not the cart's top-up: crediting it must not send the buyer to pay the basket.
    expect(takeTopUpReturn()).toBeNull();
  });

  it("continuing the very order Buy now's top-up created keeps its way back to the item", async () => {
    const attempt = stashBuyNowTopUp({ path: "/shop/item/0xc0/7", account: BOOK.buyer, lineId: "line-7", credits: 12 });
    bindBuyNowTopUp(attempt, "o-1");
    const { openExternal } = renderCredits({
      handlers: {
        userCreditOrders: () => ({ orders: [{ id: "o-1", credits: 100, usdCents: 1199, status: "initiated", createdAt: Date.now() }], total: 1 }),
        resumePackOrder: () => ({ url: "https://checkout.stripe.com/c/pay/again" }),
      },
    });
    const order = await screen.findByTestId("credit-order");
    await act(async () => fireEvent.click(within(order).getByTestId("resume-order")));
    await waitFor(() => expect(openExternal).toHaveBeenCalled());
    expect(takeTopUpReturn(Date.now(), "o-1")?.path).toBe("/shop/item/0xc0/7?buy=resume");
    expect(sessionStorage.getItem("dcl-shop-buynow-return:v1")).not.toBeNull();
  });
});
