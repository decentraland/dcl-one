// Buy now's dialog over the cart's own fakes (checkout/demo): the screens upstream's BuyModal shows, and that the
// purchase goes through the cart's checkout without ever touching the buyer's cart.
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import type { CartAddInput } from "../../cart/store";
import { createDemo, DemoShell, type DemoScenario } from "../../checkout/demo";
import { RESUME_CART_KEY } from "../../checkout/purchase/cart";
import { takeBuyNowReturn, takeTopUpReturn } from "../../credits/returns";
import { BOOK, storeMint } from "../../payment/fixtures";
import BuyModal, { resetBuyNowCarts } from "./BuyModal";

const PATH = "/shop/item/0x00000000000000000000000000000000000000c0/7";

beforeEach(() => {
  sessionStorage.clear();
  resetBuyNowCarts();
});
afterEach(() => {
  sessionStorage.clear();
  resetBuyNowCarts();
});

type Opts = {
  resume?: { credits: number; lineId?: string; account?: string };
  strict?: boolean;
  tweak?: (demo: ReturnType<typeof createDemo>) => void;
  onBuyMixed?: (mixed: unknown) => void;
};

function renderBuy(s: Omit<DemoScenario, "lines">, opts: Opts = {}) {
  const demo = createDemo({ ...s, lines: [{ offer: (x) => storeMint(x), name: "Rainbow Hat", rarity: "epic", category: "wearable" }] });
  opts.tweak?.(demo);
  const first = demo.lines[0]!;
  const line: CartAddInput = { asset: first.asset, listing: first.listing, display: first.display, seen: first.seen };
  // The buyer's own cart holds nothing, and Buy now never puts anything in it.
  demo.cart.clear();
  const opened: string[] = [];
  demo.services.openExternal = (url) => void opened.push(url);
  let closed = 0;
  const resume = opts.resume ? { account: opts.resume.account ?? BOOK.buyer.toLowerCase(), lineId: opts.resume.lineId ?? first.id, credits: opts.resume.credits } : null;
  const dialog = (
    <DemoShell demo={demo} path={PATH} routes={{ "/shop/my-items": <p>MY ITEMS</p> }}>
      <BuyModal line={line} returnPath={PATH} resume={resume} onBuyMixed={opts.onBuyMixed} onClose={() => (closed += 1)} />
    </DemoShell>
  );
  render(opts.strict ? <StrictMode>{dialog}</StrictMode> : dialog);
  return { demo, opened, closed: () => closed, lineId: first.id };
}

async function settledDialog(): Promise<HTMLElement> {
  const dialog = await screen.findByTestId("buy-modal");
  await waitFor(() => expect(dialog.querySelector("[data-phase]")?.getAttribute("data-phase")).not.toBe("loading"));
  return dialog;
}

const authorizations = (demo: ReturnType<typeof createDemo>) => demo.shop.log.filter((r) => r.name === "authorizeCreditBatch");

async function pressBuy(dialog: HTMLElement) {
  const buy = within(dialog).getByTestId("buy-modal-buy");
  await waitFor(() => expect(buy).toBeEnabled());
  await act(async () => fireEvent.click(buy));
}

describe("Buy now's dialog", () => {
  it("short on Credits with no MANA to pay: Buy Credits and Item, the shortfall, the packs that finish it, and Stripe", async () => {
    const { demo, opened, lineId } = renderBuy({ session: "web2", balanceCents: 30 });
    const dialog = await settledDialog();
    expect(within(dialog).getByRole("heading", { name: "Buy Credits and Item" })).toBeTruthy();
    expect(within(dialog).getByTestId("buy-modal-balance")).toHaveTextContent("My Credits Balance:3");
    // 115 cents against 30: 85 short, 9 whole Credits.
    expect(within(dialog).getByTestId("buy-modal-insufficient")).toHaveTextContent("Insufficient Funds. You will need to buy 9 Credits to purchase this item. Learn More");
    expect(within(dialog).getByTestId("buy-modal-asset")).toHaveTextContent("Rainbow Hat");
    const packs = within(dialog).getAllByTestId("credit-pack");
    expect(packs.map((p) => p.textContent)).toEqual(["40($5.99)", "100($11.99)", "260($29.99)", "540($59.99)"]);
    // The cheapest pack that finishes the purchase is picked, and the total follows the pick.
    expect(packs[0]).toHaveAttribute("aria-pressed", "true");
    expect(within(dialog).getByTestId("topup-total-credits")).toHaveTextContent("40");
    fireEvent.click(packs[1]!);
    expect(within(dialog).getByTestId("topup-total-credits")).toHaveTextContent("100");
    fireEvent.click(within(dialog).getByTestId("buy-modal-topup"));
    await waitFor(() => expect(opened).toEqual(["https://checkout.stripe.com/c/pay/demo"]));
    expect(demo.shop.log.find((r) => r.name === "createPackOrder")?.body).toMatchObject({ packId: "pack_10" });
    // Back from Stripe, the credits page sends the buyer to this item for this order's grant, where this tab's record of
    // what they confirmed (this line, 12 Credits, bound to the order) lets the dialog finish.
    expect(takeTopUpReturn(Date.now(), "order-pack_10")?.path).toBe(`${PATH}?buy=resume`);
    expect(takeBuyNowReturn(PATH, BOOK.buyer, "order-pack_10")).toMatchObject({ lineId, credits: 12, orderId: "order-pack_10" });
    expect(authorizations(demo)).toHaveLength(0);
  });

  it("under StrictMode the pack checkout still opens", async () => {
    const { opened } = renderBuy({ session: "web2", balanceCents: 30 }, { strict: true });
    const dialog = await settledDialog();
    fireEvent.click(within(dialog).getByTestId("buy-modal-topup"));
    await waitFor(() => expect(opened).toEqual(["https://checkout.stripe.com/c/pay/demo"]));
  });

  it("closed while the pack checkout is being created: it never leaves, and drops the hand-off", async () => {
    let answer = () => {};
    const { opened, closed } = renderBuy({
      session: "web2",
      balanceCents: 30,
      handlers: { createPackOrder: () => new Promise((r) => (answer = () => r({ orderId: "order-late", url: "https://checkout.stripe.com/c/pay/late" }))) },
    });
    const dialog = await settledDialog();
    fireEvent.click(within(dialog).getByTestId("buy-modal-topup"));
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(closed()).toBe(1);
    expect(sessionStorage.getItem("dcl-shop-buynow-return:v1")).toBeNull();
    expect(takeTopUpReturn()).toBeNull();
    await act(async () => answer());
    expect(opened).toEqual([]);
  });

  it("enough Credits: Buy Item, and Buy settles through the cart's checkout without touching the buyer's cart", async () => {
    const { demo } = renderBuy({ session: "web2" });
    const dialog = await settledDialog();
    expect(within(dialog).getByRole("heading", { name: "Buy Item" })).toBeTruthy();
    expect(within(dialog).getByTestId("buy-modal-asset")).toHaveTextContent("Rainbow Hat");
    expect(within(dialog).queryByTestId("buy-modal-methods")).toBeNull();
    await pressBuy(dialog);
    expect(await within(dialog).findByTestId("buy-modal-complete", undefined, { timeout: 5000 })).toHaveTextContent("Your purchase was successful! You can find your item in the My Items tab.");
    expect(within(dialog).getByRole("heading", { name: "Purchase complete!" })).toBeTruthy();
    const auth = authorizations(demo)[0]!;
    expect((auth.body as { items: { usdPriceCents: number }[] }).items).toEqual([expect.objectContaining({ usdPriceCents: 115 })]);
    expect(demo.cart.getSnapshot().lines).toEqual([]);
    expect(demo.cart.getSnapshot().checkout).toBeNull();
    fireEvent.click(within(dialog).getByRole("button", { name: "My items" }));
    expect(await screen.findByText("MY ITEMS")).toBeTruthy();
  });

  it("under StrictMode the item is bought once, one unit", async () => {
    const { demo } = renderBuy({ session: "web2" }, { strict: true });
    const dialog = await settledDialog();
    await pressBuy(dialog);
    await within(dialog).findByTestId("buy-modal-complete", undefined, { timeout: 5000 });
    expect(authorizations(demo)).toHaveLength(1);
    expect((authorizations(demo)[0]!.body as { items: unknown[] }).items).toHaveLength(1);
  });

  it("a buyer holding MANA who picks it pays in MANA: no Credit is authorized", async () => {
    const { demo } = renderBuy({ session: "web3" }, { tweak: (d) => d.chain.manaBalances.set(`137:${BOOK.buyer.toLowerCase()}`, 10n ** 21n) });
    const dialog = await settledDialog();
    const methods = await within(dialog).findByTestId("buy-modal-methods");
    expect(within(methods).getByRole("radio", { name: /Credits/ })).toBeChecked();
    fireEvent.click(within(methods).getByRole("radio", { name: /MANA/ }));
    // The pick is the line's method: the screen stays while the checkout re-prices on it.
    await waitFor(() => expect(within(screen.getByTestId("buy-modal-methods")).getByRole("radio", { name: /MANA/ })).toBeChecked());
    expect(demo.cart.getSnapshot().lines).toEqual([]);
    // The pick is the line's own method in the dialog's cart, which the checkout prices and pays.
    const held = Object.keys(sessionStorage).find((k) => k.startsWith("dcl-shop-buynow:v1:") && k.includes("dcl-shop-cart:v1:"))!;
    expect((JSON.parse(sessionStorage.getItem(held)!) as { lines: { method: string | null }[] }).lines[0]!.method).toBe("mana");
    await pressBuy(dialog);
    // A self-custody wallet approves the MANA spend first, then the purchase settles in MANA.
    const approve = await screen.findByTestId("authorize-step-action", undefined, { timeout: 5000 });
    await act(async () => fireEvent.click(approve));
    expect(await screen.findByTestId("buy-modal-complete", undefined, { timeout: 5000 })).toBeTruthy();
    // Paid, and not with a Credit: the purchase went out (relayed or from the wallet) with none authorized.
    expect(demo.relay.posts.length + demo.wallet.sent.length).toBeGreaterThan(0);
    expect(authorizations(demo)).toHaveLength(0);
  });

  it("a buyer short on Credits with MANA can split: the Credits first and MANA for the rest, through the page's flow", async () => {
    const split: unknown[] = [];
    renderBuy(
      { session: "web3", balanceCents: 30 },
      { tweak: (d) => d.chain.manaBalances.set(`137:${BOOK.buyer.toLowerCase()}`, 10n ** 21n), onBuyMixed: (m) => void split.push(m) },
    );
    const dialog = await settledDialog();
    fireEvent.click(within(await within(dialog).findByTestId("buy-modal-methods")).getByRole("radio", { name: /MANA/ }));
    const option = await within(dialog).findByTestId("buy-modal-mixed");
    fireEvent.click(within(option).getByRole("checkbox"));
    fireEvent.click(within(dialog).getByTestId("buy-modal-buy"));
    expect(split).toEqual([expect.objectContaining({ credits: 3 })]);
  });

  it("back from a top-up it finishes on its own, at the price the buyer confirmed", async () => {
    const { demo } = renderBuy({ session: "web2" }, { resume: { credits: 12 } });
    expect(await screen.findByTestId("buy-modal-complete", undefined, { timeout: 5000 })).toBeTruthy();
    expect(authorizations(demo)).toHaveLength(1);
  });

  it("back from a top-up at a higher price, it shows the new price and waits for a press", async () => {
    const { demo } = renderBuy({ session: "web2" }, { resume: { credits: 10 } });
    const dialog = await settledDialog();
    expect(within(dialog).getByTestId("price-changed")).toBeTruthy();
    await act(async () => new Promise((r) => setTimeout(r, 50)));
    expect(authorizations(demo)).toHaveLength(0);
    // The press agrees to the price on screen, and that is what is charged.
    await pressBuy(dialog);
    await within(dialog).findByTestId("buy-modal-complete", undefined, { timeout: 5000 });
    expect((authorizations(demo)[0]!.body as { items: { usdPriceCents: number }[] }).items).toEqual([expect.objectContaining({ usdPriceCents: 115 })]);
  });

  it("back from Stripe through the page cache, the pack checkout can be started again", async () => {
    const { opened } = renderBuy({ session: "web2", balanceCents: 30 });
    const dialog = await settledDialog();
    fireEvent.click(within(dialog).getByTestId("buy-modal-topup"));
    await waitFor(() => expect(opened).toHaveLength(1));
    expect(within(dialog).getByTestId("buy-modal-topup")).toBeDisabled();
    act(() => {
      const shown = new Event("pageshow") as PageTransitionEvent;
      Object.defineProperty(shown, "persisted", { value: true });
      window.dispatchEvent(shown);
    });
    expect(within(dialog).getByTestId("buy-modal-topup")).toBeEnabled();
  });

  it("back from a top-up at a lower price, it finishes on its own", async () => {
    const { demo } = renderBuy({ session: "web2" }, { resume: { credits: 20 } });
    expect(await screen.findByTestId("buy-modal-complete", undefined, { timeout: 5000 })).toBeTruthy();
    expect(authorizations(demo)).toHaveLength(1);
  });

  it("back from a top-up another account confirmed, it waits for a press", async () => {
    const { demo } = renderBuy({ session: "web2" }, { resume: { credits: 12, account: "0x0000000000000000000000000000000000000002" } });
    await settledDialog();
    await act(async () => new Promise((r) => setTimeout(r, 50)));
    expect(authorizations(demo)).toHaveLength(0);
  });

  it("back from a top-up for another line, it waits for a press", async () => {
    const { demo } = renderBuy({ session: "web2" }, { resume: { credits: 12, lineId: "another-line" } });
    await settledDialog();
    await act(async () => new Promise((r) => setTimeout(r, 50)));
    expect(authorizations(demo)).toHaveLength(0);
  });

  it("never takes the cart's own resume hand-off", async () => {
    sessionStorage.setItem(RESUME_CART_KEY, "1");
    renderBuy({ session: "web2" });
    await settledDialog();
    expect(sessionStorage.getItem(RESUME_CART_KEY)).toBe("1");
  });

  it("a purchase not confirmed yet says so, can be checked again, and can be closed", async () => {
    const { closed } = renderBuy({ session: "web2", confirm: "none" });
    const dialog = await settledDialog();
    await pressBuy(dialog);
    const pending = await within(dialog).findByTestId("buy-modal-pending", undefined, { timeout: 5000 });
    expect(pending).toHaveTextContent("Your purchase is still being confirmed.");
    expect(within(pending).getByRole("button", { name: "Check again" })).toBeTruthy();
    fireEvent.click(within(pending).getByRole("button", { name: "Close" }));
    expect(closed()).toBe(1);
  });

  it("a purchase the relay refuses ends on the cart's failure panel, with a retry and a way out", async () => {
    const { closed } = renderBuy({ session: "web2", relay: "refuse" });
    const dialog = await settledDialog();
    await pressBuy(dialog);
    expect(await within(dialog).findByTestId("buy-error", undefined, { timeout: 5000 })).toHaveTextContent("We couldn't complete your purchase this time.");
    expect(within(dialog).getByRole("heading", { name: "Oops! Something went wrong" })).toBeTruthy();
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(closed()).toBe(1);
  });

  it("closes from its X, its scrim and Escape while nothing is being paid", async () => {
    const { closed } = renderBuy({ session: "web2" });
    const dialog = await settledDialog();
    fireEvent.click(within(dialog).getByTestId("buy-modal-close"));
    fireEvent.keyDown(window, { key: "Escape" });
    fireEvent.click(within(dialog).getByTestId("buy-modal-scrim"));
    expect(closed()).toBe(3);
  });
});
