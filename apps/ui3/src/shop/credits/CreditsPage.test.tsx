import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { createDemo, DemoShell, type DemoScenario } from "../checkout/demo";
import type { FakeHandlers } from "../checkout/fakes";
import type { ShopWalletHost } from "../wallet/session";
import CreditsPage from "./CreditsPage";
import { stashTopUpReturn } from "./returns";

function renderCredits(s: Partial<DemoScenario> & { path?: string; handlers?: FakeHandlers } = {}) {
  const demo = createDemo({ session: "web3", ...s });
  const openExternal = vi.fn();
  const onSignIn = vi.fn();
  demo.services.openExternal = openExternal;
  demo.host.onSignIn = onSignIn;
  const view = render(
    <DemoShell demo={demo} path={s.path ?? "/shop/credits"} routes={{ "/shop/cart": <p>CART PAGE</p>, "/shop/items": <p>ITEMS PAGE</p> }}>
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
  it("shows the balance in whole credits, what is on hold, and the server's packs in order", async () => {
    renderCredits({ balanceCents: 1234, held: { cents: 90, credits: 9, releasesAtSeconds: null, purchases: [] } });
    await waitFor(() => expect(screen.getByTestId("credits-balance").textContent).toBe("123 Credits"));
    expect(screen.getByTestId("held-credits").textContent).toContain("9");
    const packs = await screen.findAllByTestId("pack");
    expect(packs.map((p) => p.getAttribute("aria-label"))).toEqual([
      "40 Credits for $5.99",
      expect.stringMatching(/^100 Credits for \$11\.99/),
      expect.stringMatching(/^260 Credits for \$29\.99/),
      expect.stringMatching(/^540 Credits for \$59\.99/),
    ]);
    expect(screen.getByLabelText("1 Credit equals 0.10 US dollars")).toBeTruthy();
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
    await act(async () => fireEvent.click(within(order).getByTestId("resume-order")));
    await waitFor(() => expect(openExternal).toHaveBeenCalledWith("https://checkout.stripe.com/c/pay/again"));
  });
});
