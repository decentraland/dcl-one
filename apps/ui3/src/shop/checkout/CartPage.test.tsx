import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { newCheckout } from "../cart/machine";
import { partitionCart } from "../cart/partition";
import { rarityCardPalette } from "../discovery/lib/rarity";
import { creatorOrder, storeMint, wearableResale, parcelResale, BOOK } from "../payment/fixtures";
import CartPage from "./CartPage";
import { creatorTrade } from "./fakes";
import { createDemo, DemoShell, type DemoScenario } from "./demo";
import { RESUME_CART_KEY } from "./purchase/cart";

function renderCart(s: DemoScenario, tweak?: (demo: ReturnType<typeof createDemo>) => void) {
  const demo = createDemo(s);
  tweak?.(demo);
  const view = render(
    <DemoShell demo={demo} routes={{ "/shop/success": <p>SUCCESS PAGE</p>, "/shop/credits": <p>CREDITS PAGE</p> }}>
      <CartPage />
    </DemoShell>,
  );
  return { demo, ...view };
}

const MIXED: DemoScenario["lines"] = [
  { offer: (s) => storeMint(s), name: "Rainbow Hat" },
  { offer: (s) => wearableResale(s), listing: { tradeId: "trade-resale-1" }, name: "Vintage Jacket" },
];

const walletSettled = () => act(async () => {});

// The summary's primary rail, as upstream's PaymentCtas: the credits button when the balance covers the
// cart, else the credits fallback ("Checkout", which opens a top-up when the balance falls short).
const confirmButton = () => screen.queryByTestId("pay-with-credits") ?? screen.getByTestId("pay-with-credits-topup");

// The wallet reports its account and chain after the first price check, and that re-quotes the cart
// (wallet.revision), so a button can turn enabled, disabled, enabled. Check and click in one
// synchronous step so the click cannot land on the disabled render in between.
async function clickWhenEnabled(button: () => HTMLElement) {
  await waitFor(() => {
    expect(button()).toBeEnabled();
    fireEvent.click(button());
  });
  await act(async () => {});
}

describe("the cart line, drawn as upstream's", () => {
  it("one ready way to pay needs no chooser", async () => {
    renderCart({ session: "web2", lines: [MIXED![0]!] });
    const mint = screen.getByRole("article", { name: "Rainbow Hat" });
    expect(within(mint).queryByTestId("payment-choice")).toBeNull();
    await walletSettled();
  });

  it("a saved method the session no longer offers keeps the chooser, so the buyer can pick again", async () => {
    renderCart({ session: "web2", lines: [{ ...MIXED![0]!, method: "mana" }] });
    const mint = screen.getByRole("article", { name: "Rainbow Hat" });
    expect(within(mint).getByRole("radio", { name: /Credits/ })).toBeTruthy();
    await walletSettled();
  });

  it("the line price and the total are read with their unit", async () => {
    renderCart({ session: "web2", lines: [MIXED![0]!] });
    const mint = screen.getByRole("article", { name: "Rainbow Hat" });
    expect(within(mint).getByText("12 Credits", { selector: ".shop-sr" })).toBeTruthy();
    expect(within(screen.getByTestId("total-credits")).getByText("12 Credits", { selector: ".shop-sr" })).toBeTruthy();
    await walletSettled();
  });

  it("back from a cart opened first (a link, a new tab, the Explorer's memory router) goes to the Shop", async () => {
    const demo = createDemo({ session: "web2", lines: [MIXED![0]!] });
    render(
      <DemoShell demo={demo} routes={{ "/shop/items": <p>ITEMS PAGE</p> }}>
        <CartPage />
      </DemoShell>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Back from the Cart" }));
    expect(await screen.findByText("ITEMS PAGE")).toBeTruthy();
  });

  it("signed out, the mint's only option waits on sign-in, so the chooser stays hidden too", async () => {
    renderCart({ session: "signedOut", lines: [MIXED![0]!] });
    expect(within(screen.getByRole("article", { name: "Rainbow Hat" })).queryByTestId("payment-choice")).toBeNull();
    await walletSettled();
  });

  it("signed out, the summary shows no confirmation group: there is nothing to confirm before sign-in", async () => {
    renderCart({ session: "signedOut", lines: MIXED });
    await walletSettled();
    expect(screen.queryAllByTestId("checkout-group")).toEqual([]);
    expect(screen.getByText("You'll be asked to sign in first.")).toBeTruthy();
  });

  it("the thumbnail is the rarity's tile with its category flap, and the band opens with a back link", async () => {
    renderCart({ session: "web3", lines: [{ ...MIXED![0]!, category: "wearable", wearableCategory: "hat", rarity: "epic" }] });
    const mint = screen.getByRole("article", { name: "Rainbow Hat" });
    const thumb = mint.querySelector<HTMLElement>("[data-thumb]")!;
    expect(thumb.style.getPropertyValue("--tile-stroke")).toBe(rarityCardPalette("epic").stroke);
    expect(thumb.style.getPropertyValue("--tile-bg")).toBe(rarityCardPalette("epic").background);
    expect(within(mint).getByTestId("cart-line-flap").getAttribute("data-size")).toBe("sm");
    expect(within(screen.getByRole("article", { name: "Rainbow Hat" })).getByTestId("cart-line-flap-icon")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Back from the Cart" })).toBeTruthy();
    await walletSettled();
  });
});

describe("the cart page", () => {
  it("empty: says so and links to the catalogue", async () => {
    renderCart({ session: "web3", lines: [] });
    expect(screen.getByTestId("cart-empty").textContent).toContain("Nothing here... yet!");
    expect(screen.getByRole("link", { name: "Start shopping" }).getAttribute("href")).toBe("/shop/items");
    await walletSettled();
  });

  it("web3 mixed cart: credits first on the mint, MANA only on the resale, separate totals and groups, then both groups settle", async () => {
    const { demo } = renderCart({ session: "web3", lines: MIXED });
    const mint = screen.getByRole("article", { name: "Rainbow Hat" });
    const resale = screen.getByRole("article", { name: "Vintage Jacket" });
    // Upstream's line has no chooser; the resale keeps its MANA-only reason.
    expect(within(mint).queryByTestId("payment-choice")).toBeNull();
    expect(within(resale).queryByRole("radio")).toBeNull();
    expect(within(resale).getByTestId("payment-mana-only").textContent).toContain("This is a resale: the seller is paid in MANA.");
    expect(within(resale).getByTestId("payment-mana-only").textContent).not.toMatch(/credit/i);
    expect(screen.getByTestId("total-credits").textContent).toContain("12");
    expect(screen.getByTestId("total-mana-137").textContent).toMatch(/MANA on Polygon\s*2\s*MANA/);
    // No group before checkout, as upstream; the resale takes the credits rail away, so the button is Checkout.
    expect(screen.queryAllByTestId("checkout-group")).toHaveLength(0);
    expect(screen.queryByTestId("pay-with-credits")).toBeNull();
    expect(screen.getByTestId("pay-with-credits-topup").textContent).toBe("Checkout");

    await clickWhenEnabled(confirmButton);
    const approve = await screen.findByTestId("authorize-step-action");
    expect(screen.getByTestId("authorize-step").textContent).toContain("OffChainMarketplaceV3");
    await act(async () => fireEvent.click(approve));
    await screen.findByText("SUCCESS PAGE");
    expect(demo.cart.getSnapshot().lines).toEqual([]);
    expect(demo.cart.getSnapshot().checkout).toBeNull();
    const auth = demo.shop.log.find((r) => r.name === "authorizeCreditBatch")!;
    expect((auth.body as { items: { usdPriceCents: number }[] }).items).toEqual([{ usdPriceCents: 115, contractAddress: BOOK.collection, itemId: "7" }]);
  });

  it("a covered credits cart pays from one Checkout button, and the line has no chooser", async () => {
    renderCart({ session: "web3", lines: [{ offer: (s) => storeMint(s), name: "Rainbow Hat" }] });
    await waitFor(() => expect(screen.getByTestId("pay-with-credits")).toBeEnabled());
    expect(screen.getByTestId("pay-with-credits").textContent).toBe("Checkout");
    expect(within(screen.getByRole("article", { name: "Rainbow Hat" })).queryByTestId("payment-choice")).toBeNull();
    expect(screen.queryByText(/Your Credits/)).toBeNull();
  });

  it("quantities: a mint steps up to the server's stock; a token has no stepper", async () => {
    const { demo } = renderCart({ session: "web3", lines: MIXED });
    const mint = screen.getByRole("article", { name: "Rainbow Hat" });
    fireEvent.click(within(mint).getByRole("button", { name: "More Rainbow Hat" }));
    expect(demo.cart.getSnapshot().lines[0]!.quantity).toBe(2);
    expect(screen.getByTestId("total-credits").textContent).toContain("24");
    expect(within(screen.getByRole("article", { name: "Vintage Jacket" })).queryByRole("group", { name: /Quantity/ })).toBeNull();
    await walletSettled();
  });

  it("price-check outage: the choices stay, the reason is shown, nothing can be bought", async () => {
    renderCart({ session: "web3", lines: MIXED, offers: "outage" });
    expect(screen.getByRole("alert", { name: "" }).textContent ?? "").toBeDefined();
    expect(screen.getByText("Prices can't be checked right now")).toBeTruthy();
    expect(confirmButton()).toBeDisabled();
    await walletSettled();
  });

  it("no quote route and lines added from a card: each line still shows the rule's choices, unavailable, never 'Checking'", () => {
    const fromCards: DemoScenario["lines"] = MIXED!.map((l) => ({ ...l, category: "wearable", unseen: true }));
    for (const session of ["web3", "web2"] as const) {
      const { unmount } = renderCart({ session, lines: fromCards, offers: "outage" });
      expect(screen.queryByText("Checking the price…")).toBeNull();
      const mint = screen.getByRole("article", { name: "Rainbow Hat" });
      expect(within(mint).getByRole("radio", { name: /Credits/ })).toBeChecked();
      expect(within(mint).queryAllByRole("radio", { name: /MANA/ })).toHaveLength(session === "web3" ? 1 : 0);
      expect(within(mint).getAllByText("The Credits price is unavailable right now. Try again in a moment.")).toHaveLength(1);
      const resale = screen.getByRole("article", { name: "Vintage Jacket" });
      expect(within(resale).getByTestId("payment-mana-only").textContent).toContain("This is a resale: the seller is paid in MANA.");
      expect(within(resale).getByTestId("payment-mana-only").textContent).not.toMatch(/credit/i);
      if (session === "web2") expect(within(resale).getByRole("button", { name: "How to pay with MANA" })).toBeTruthy();
      expect(confirmButton()).toBeDisabled();
      unmount();
    }
  });

  it("the quote route failing is a retryable outage, never a switch of method", async () => {
    renderCart({ session: "web3", lines: MIXED, offers: "failing" });
    await screen.findByRole("button", { name: "Try again" });
    expect(confirmButton()).toBeDisabled();
    expect(screen.queryByTestId("pay-with-mana")).toBeNull();
  });

  it("a Credits quote outage is an outage with a retry, said once per line, never a change to accept or a method to re-choose", async () => {
    const outage = (s: Parameters<typeof storeMint>[0]) => {
      const offer = storeMint(s);
      return { ...offer, quotes: offer.quotes.filter((q) => q.method !== "credits"), unavailable: [{ method: "credits" as const, missing: "manaUsdRate" as const }] };
    };
    renderCart({ session: "web2", lines: [{ offer: outage, name: "Rainbow Hat" }, MIXED[1]!] });
    await screen.findByText("The Credits price is unavailable right now");
    expect(screen.queryByText("Something changed since you added it")).toBeNull();
    const blockers = within(screen.getByRole("list", { name: "Why you can't confirm yet" })).getAllByRole("listitem").map((li) => li.textContent);
    expect(blockers).toContain("The Credits price is unavailable right now. Try again in a moment.");
    expect(blockers.join(" ")).not.toMatch(/Choose how to pay|Prices or availability changed|Accept the new prices/);
    const mint = screen.getByRole("article", { name: "Rainbow Hat" });
    expect(within(mint).getAllByText(/The Credits price is unavailable right now/)).toHaveLength(1);
    expect(confirmButton()).toBeDisabled();
  });

  it("stale price: the change is shown and must be accepted before confirming", async () => {
    renderCart({ session: "web3", lines: MIXED, offers: "repriced" });
    await screen.findByRole("button", { name: "Accept the changes" });
    expect(await within(screen.getByRole("article", { name: "Rainbow Hat" })).findByText(/Price changed from 12 Credits to 14 Credits/)).toBeTruthy();
    expect(confirmButton()).toBeDisabled();
    fireEvent.click(await screen.findByRole("button", { name: "Accept the changes" }));
    await waitFor(() => expect(confirmButton()).toBeEnabled());
  });

  it("sold listing: the line says it is gone and the cart cannot be confirmed with it", async () => {
    renderCart({ session: "web3", lines: MIXED, offers: "sold" });
    await within(screen.getByRole("article", { name: "Rainbow Hat" })).findByText("No longer available: sold or removed.");
    expect(confirmButton()).toBeDisabled();
  });

  it("web2: the resale is MANA only with wallet guidance that never mentions credits; its group needs a wallet", async () => {
    renderCart({ session: "web2", lines: MIXED });
    const resale = screen.getByRole("article", { name: "Vintage Jacket" });
    fireEvent.click(within(resale).getByRole("button", { name: "How to pay with MANA" }));
    const guidance = within(resale).getByRole("heading", { name: "This purchase is paid in MANA" }).closest("section")!;
    expect(guidance.textContent).toContain("Polygon");
    expect(guidance.textContent).not.toMatch(/credit/i);
    expect(within(screen.getByRole("article", { name: "Rainbow Hat" })).queryByRole("radio", { name: /MANA/ })).toBeNull();
    expect(screen.queryByTestId("pay-with-mana")).toBeNull();
    await waitFor(() => expect(confirmButton()).toBeEnabled());
    // Before any checkout, the one group this session cannot pay still says why.
    expect(screen.getAllByTestId("checkout-group")[1]!.textContent).toContain("Needs a web3 wallet with MANA");
  });

  it("the MANA rail moves the cart to MANA and checks out only once the cart has re-priced on it", async () => {
    let release = () => {};
    const gate = new Promise<void>((r) => (release = r));
    const { demo } = renderCart({ session: "web3", lines: [{ offer: (s) => creatorOrder(s), listing: { tradeId: "trade-creator-1" }, name: "Order" }] }, (d) => {
      d.chain.manaBalances.set(`137:${BOOK.buyer.toLowerCase()}`, 10n ** 21n);
      const fetchOffers = d.services.fetchOffers!;
      // The MANA re-price waits on the test, so a checkout that does not wait for it would start first.
      d.services.fetchOffers = async (lines, signal) => {
        if (lines.some((l) => l.method === "mana")) await gate;
        return fetchOffers(lines, signal);
      };
    });
    await waitFor(() => expect(screen.getByTestId("pay-with-mana")).toBeEnabled());
    expect(screen.getByTestId("pay-with-credits").textContent).toBe("Buy with Credits");
    await clickWhenEnabled(() => screen.getByTestId("pay-with-mana"));
    expect(demo.cart.getSnapshot().lines[0]!.method).toBe("mana");
    expect(screen.queryByTestId("authorize-step-action")).toBeNull();
    expect(screen.getByTestId("pay-with-mana")).toBeDisabled();
    await act(async () => release());
    expect(await screen.findByTestId("authorize-step-action")).toBeTruthy();
    expect(demo.shop.log.some((r) => r.name === "authorizeCreditBatch")).toBe(false);
  });

  it("the Credits rail on lines left on MANA re-prices them and stops: the buyer sees the Credits total before paying", async () => {
    const order = { offer: (s: Parameters<typeof creatorOrder>[0]) => creatorOrder(s), listing: { tradeId: "trade-creator-1" }, name: "Order", method: "mana" as const };
    const { demo } = renderCart({ session: "web3", lines: [order] }, (d) => d.chain.manaBalances.set(`137:${BOOK.buyer.toLowerCase()}`, 10n ** 21n));
    await waitFor(() => expect(screen.getByTestId("pay-with-credits")).toBeEnabled());
    await clickWhenEnabled(() => screen.getByTestId("pay-with-credits"));
    expect(demo.cart.getSnapshot().lines[0]!.method).toBe("credits");
    expect(await screen.findByTestId("cart-repriced")).toBeTruthy();
    expect(demo.shop.log.some((r) => r.name === "authorizeCreditBatch")).toBe(false);
    // The second press pays what is now on screen.
    await clickWhenEnabled(() => screen.getByTestId("pay-with-credits"));
    await waitFor(() => expect(demo.shop.log.some((r) => r.name === "authorizeCreditBatch")).toBe(true));
  });

  it("a MANA price that moved during the rail's re-price stops the checkout on the change, with nothing authorized", async () => {
    const { demo } = renderCart({ session: "web3", lines: [{ offer: (s) => creatorOrder(s), listing: { tradeId: "trade-creator-1" }, name: "Order" }] }, (d) => {
      d.chain.manaBalances.set(`137:${BOOK.buyer.toLowerCase()}`, 10n ** 21n);
      const fetchOffers = d.services.fetchOffers!;
      d.services.fetchOffers = async (lines, signal) => {
        const out = await fetchOffers(lines, signal);
        if (!lines.some((l) => l.method === "mana")) return out;
        return Object.fromEntries(
          Object.entries(out).map(([id, o]) => [id, o && { ...o, quotes: o.quotes.map((q) => (q.method === "mana" ? { ...q, amount: (q.amount + 10n ** 17n) as typeof q.amount } : q)) }]),
        );
      };
    });
    await clickWhenEnabled(() => screen.getByTestId("pay-with-mana"));
    expect(await screen.findByRole("button", { name: "Accept the changes" })).toBeTruthy();
    expect(screen.queryByTestId("authorize-step-action")).toBeNull();
    expect(demo.wallet.requests.some((r) => r.method === "eth_sendTransaction")).toBe(false);
  });

  it("an outage during the rail's re-price ends the wait: the outage is said and nothing is bought", async () => {
    const { demo } = renderCart({ session: "web3", lines: [{ offer: (s) => creatorOrder(s), listing: { tradeId: "trade-creator-1" }, name: "Order" }] }, (d) => {
      d.chain.manaBalances.set(`137:${BOOK.buyer.toLowerCase()}`, 10n ** 21n);
      const fetchOffers = d.services.fetchOffers!;
      d.services.fetchOffers = async (lines, signal) => {
        if (lines.some((l) => l.method === "mana")) throw new Error("quote route unavailable");
        return fetchOffers(lines, signal);
      };
    });
    await clickWhenEnabled(() => screen.getByTestId("pay-with-mana"));
    await screen.findByRole("button", { name: "Try again" });
    // The wait is over: no button still says it is working.
    await waitFor(() => expect(screen.queryByText("Working…")).toBeNull());
    expect(screen.queryByTestId("authorize-step-action")).toBeNull();
    expect(demo.shop.log.some((r) => r.name === "authorizeCreditBatch")).toBe(false);
  });

  it("MANA the wallet holds but too little of: a disabled MANA button with its reason attached", async () => {
    renderCart({ session: "web3", lines: [{ offer: (s) => creatorOrder(s), listing: { tradeId: "trade-creator-1" }, name: "Order" }] }, (d) => d.chain.manaBalances.set(`137:${BOOK.buyer.toLowerCase()}`, 1n));
    const disabled = await screen.findByTestId("pay-with-mana-disabled");
    expect(disabled).toBeDisabled();
    const note = screen.getByTestId("mana-shortfall-note");
    expect(note.textContent).toBe("Not enough MANA");
    expect(disabled.getAttribute("aria-describedby")).toBe(note.id);
  });

  it("a balance that cannot be read says so and offers no credits fallback to guess with", async () => {
    renderCart({ session: "web3", lines: [{ offer: (s) => storeMint(s), name: "Rainbow Hat" }], handlers: { userCredits: () => ({ status: 500, body: { error: "down" } }) } });
    expect(await screen.findByTestId("cart-balance-unknown")).toBeTruthy();
    expect(screen.queryByTestId("pay-with-credits")).toBeNull();
    expect(screen.queryByTestId("pay-with-credits-topup")).toBeNull();
  });

  it("a cart that changes under the rail's wait ends the wait: the lines are locked while it runs, and nothing is bought", async () => {
    let release = () => {};
    const gate = new Promise<void>((r) => (release = r));
    const { demo } = renderCart({ session: "web3", lines: [{ offer: (s) => creatorOrder(s), listing: { tradeId: "trade-creator-1" }, name: "Order" }] }, (d) => {
      d.chain.manaBalances.set(`137:${BOOK.buyer.toLowerCase()}`, 10n ** 21n);
      const fetchOffers = d.services.fetchOffers!;
      d.services.fetchOffers = async (lines, signal) => {
        if (lines.some((l) => l.method === "mana")) await gate;
        return fetchOffers(lines, signal);
      };
    });
    await clickWhenEnabled(() => screen.getByTestId("pay-with-mana"));
    expect(within(screen.getByRole("article", { name: "Order" })).getByRole("button", { name: /Remove/ })).toBeDisabled();
    // Another tab (or an account switch) changes the cart while the re-price is out.
    const id = demo.cart.getSnapshot().lines[0]!.id;
    await act(async () => demo.cart.setQuantity(id, 2));
    await waitFor(() => expect(screen.queryByText("Working…")).toBeNull());
    await act(async () => release());
    await walletSettled();
    expect(screen.queryByTestId("authorize-step-action")).toBeNull();
  });

  it("a rail pressed while a sign-in resume is armed starts one checkout, not two", async () => {
    sessionStorage.setItem(RESUME_CART_KEY, "1");
    let settled = 0;
    try {
      renderCart({
        session: "web3",
        lines: [{ offer: (s) => creatorOrder(s), listing: { tradeId: "trade-creator-1" }, name: "Order" }],
        balanceCents: 30,
        resolvers: {
          trade: async () => {
            settled++;
            return creatorTrade();
          },
        },
      }, (d) => d.chain.manaBalances.set(`137:${BOOK.buyer.toLowerCase()}`, 10n ** 21n));
      await clickWhenEnabled(() => screen.getByTestId("pay-with-mana"));
      expect(await screen.findByTestId("authorize-step-action")).toBeTruthy();
      await walletSettled();
      expect(settled).toBe(1);
    } finally {
      sessionStorage.removeItem(RESUME_CART_KEY);
    }
  });

  it("the MANA rail on lines already on MANA pays at once: the MANA total is the one on screen", async () => {
    renderCart({ session: "web3", lines: [{ offer: (s) => creatorOrder(s), listing: { tradeId: "trade-creator-1" }, name: "Order", method: "mana" }] }, (d) =>
      d.chain.manaBalances.set(`137:${BOOK.buyer.toLowerCase()}`, 10n ** 21n),
    );
    await clickWhenEnabled(() => screen.getByTestId("pay-with-mana"));
    expect(await screen.findByTestId("authorize-step-action")).toBeTruthy();
    expect(screen.queryByTestId("cart-repriced")).toBeNull();
  });

  it("lines left on MANA with too few credits: Checkout puts them back on credits and offers the top-up, never a silent no-op", async () => {
    const { demo } = renderCart({ session: "web3", lines: [{ offer: (s) => creatorOrder(s), listing: { tradeId: "trade-creator-1" }, name: "Order", method: "mana" }], balanceCents: 30 });
    // Neither balance pays: no credits rail (whatever the line is set to), no MANA held, so the fallback stays.
    await waitFor(() => expect(screen.getByTestId("pay-with-credits-topup")).toBeEnabled());
    expect(screen.queryByTestId("pay-with-credits")).toBeNull();
    expect(screen.queryByTestId("pay-with-mana")).toBeNull();
    await clickWhenEnabled(() => screen.getByTestId("pay-with-credits-topup"));
    expect(await screen.findByTestId("topup-panel")).toBeTruthy();
    expect(demo.cart.getSnapshot().lines[0]!.method).toBeNull();
  });

  it("web2 with nothing to sign: the blocker asks to sign in again, never for a web3 wallet", async () => {
    renderCart({ session: "web2", lines: [{ offer: (s) => storeMint(s), name: "Rainbow Hat" }], noSigner: true });
    const blockers = await screen.findByRole("list", { name: "Why you can't confirm yet" });
    await waitFor(() => expect(blockers.textContent).toContain("Your sign-in can't confirm purchases right now. Sign in again to continue."));
    expect(blockers.textContent).not.toMatch(/wallet/i);
    expect(confirmButton()).toBeDisabled();
  });

  it("insufficient credits: an explicit top-up with packs that close the gap, the line stays on credits", async () => {
    const { demo } = renderCart({ session: "web3", lines: [{ offer: (s) => storeMint(s), name: "Rainbow Hat", quantity: 5 }], balanceCents: 30 });
    // As upstream: no inline shortfall; Checkout stays enabled and opens the top-up.
    await waitFor(() => expect(screen.getByTestId("pay-with-credits-topup")).toBeEnabled());
    expect(screen.queryByText(/You need 57 more Credits/)).toBeNull();
    await clickWhenEnabled(() => screen.getByTestId("pay-with-credits-topup"));
    const panel = await screen.findByTestId("topup-panel");
    expect(panel.textContent).toContain("You need 57 more Credits");
    expect(within(panel).getAllByTestId("credit-pack").map((b) => b.getAttribute("aria-label"))).toEqual([
      expect.stringContaining("100 Credits"),
      expect.stringContaining("260 Credits"),
      expect.stringContaining("540 Credits"),
    ]);
    expect(within(panel).getByTestId("pack-recommended")).toBeTruthy();
    expect(demo.cart.getSnapshot().lines[0]!.method).toBeNull();
    expect(panel.textContent).not.toMatch(/MANA/);
  });

  it("a balance a few cents short says the whole Credits the buyer must add, rounded up", async () => {
    renderCart({ session: "web3", lines: [{ offer: (s) => storeMint(s), name: "Rainbow Hat" }], balanceCents: 115 });
    await clickWhenEnabled(() => screen.getByTestId("pay-with-credits-topup"));
    expect((await screen.findByTestId("topup-panel")).textContent).toContain("You need 1 more Credits");
  });

  it("wallet rejection: the group fails as cancelled, nothing is bought, and the checkout can resume", async () => {
    const { demo } = renderCart({ session: "web3", lines: [{ offer: (s) => storeMint(s), name: "Rainbow Hat" }] });
    await waitFor(() => expect(confirmButton()).toBeEnabled());
    demo.wallet.next = "reject";
    await act(async () => fireEvent.click(confirmButton()));
    await screen.findByText("You cancelled the request in your wallet.");
    expect(demo.cart.getSnapshot().lines).toHaveLength(1);
    expect(screen.getByText("A purchase from this cart didn't finish")).toBeTruthy();
    expect(demo.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(true);
    await act(async () => fireEvent.click(screen.getByTestId("resume-checkout")));
    await screen.findByText("SUCCESS PAGE");
  });

  it("a revert (sold between review and settlement) fails the group without taking the line out", async () => {
    const { demo } = renderCart({ session: "web3", lines: [{ offer: (s) => storeMint(s), name: "Rainbow Hat" }], confirm: "reverted" });
    await clickWhenEnabled(confirmButton);
    await screen.findByText(/The transaction failed on-chain/);
    expect(demo.cart.getSnapshot().lines).toHaveLength(1);
  });

  it("process death: a reloaded cart finds its pending group, watches it, and a late receipt completes it", async () => {
    const hash = `0x${"e".repeat(64)}`;
    let clock = 0;
    let gaveUp!: () => void;
    const watchGaveUp = new Promise<void>((resolve) => (gaveUp = resolve));
    const wait = {
      timeoutMs: 40,
      pollMs: 10,
      now: () => {
        if (clock >= 40) gaveUp();
        return clock;
      },
      sleep: async (ms: number) => {
        clock += ms;
      },
    };
    const { demo } = renderCart({
      session: "web3",
      lines: [{ offer: (s) => storeMint(s), name: "Rainbow Hat" }],
      checkout: (lines, cart) => {
        const offer = storeMint({ kind: "web3", wallet: BOOK.buyer });
        const part = partitionCart([{ line: cart.getSnapshot().lines[0]!, offer }], { kind: "web3", wallet: BOOK.buyer });
        const state = newCheckout(BOOK.buyer, part.groups, Date.now(), "reloaded");
        state.groups[0] = { ...state.groups[0]!, status: "pending", txHash: hash, creditIds: ["0xabc"], attempts: 1 };
        void lines;
        return state;
      },
    }, (d) => {
      d.services.wait = wait;
    });
    expect(screen.getByText("A purchase from this cart is still being confirmed")).toBeTruthy();
    await act(() => watchGaveUp);
    expect(demo.cart.getSnapshot().checkout!.groups[0]!.status).toBe("pending");
    expect(screen.queryByRole("button", { name: "Start over" }), "a broadcast group can never be discarded").toBeNull();
    demo.chain.receipts.set(hash, "success");
    await act(async () => fireEvent.click(screen.getByTestId("resume-checkout")));
    await waitFor(() => expect(demo.cart.getSnapshot().lines).toHaveLength(0));
    expect(demo.relay.posts).toHaveLength(0);
  });

  function seeded(status: "failed" | "authorizing", creditIds: string[] = []): DemoScenario["checkout"] {
    return (_lines, cart) => {
      const offer = storeMint({ kind: "web3", wallet: BOOK.buyer });
      const part = partitionCart([{ line: cart.getSnapshot().lines[0]!, offer }], { kind: "web3", wallet: BOOK.buyer });
      const state = newCheckout(BOOK.buyer, part.groups, Date.now(), "earlier");
      state.groups[0] = {
        ...state.groups[0]!,
        status,
        creditIds,
        attempts: 1,
        failure: status === "failed" ? { kind: "rejected", message: "You cancelled the request in your wallet." } : null,
      };
      return state;
    };
  }

  it("resume runs no group at a price the buyer has not accepted", async () => {
    const { demo } = renderCart({ session: "web3", lines: [{ offer: (s) => storeMint(s), name: "Rainbow Hat" }], offers: "repriced", checkout: seeded("failed") });
    await screen.findByText("Something changed since you added it");
    expect(screen.getByTestId("resume-checkout")).toBeDisabled();
    await act(async () => fireEvent.click(screen.getByTestId("resume-checkout")));
    expect(demo.shop.log.some((r) => r.name === "authorizeCreditBatch")).toBe(false);
    fireEvent.click(await screen.findByRole("button", { name: "Accept the changes" }));
    await clickWhenEnabled(() => screen.getByTestId("resume-checkout"));
    await screen.findByText("SUCCESS PAGE", {}, { timeout: 5000 });
    const auth = demo.shop.log.find((r) => r.name === "authorizeCreditBatch")!;
    expect((auth.body as { items: { usdPriceCents: number }[] }).items[0]!.usdPriceCents).toBe(135);
  });

  it("resume is refused while the wallet is on another account than the signed session", async () => {
    const { demo } = renderCart({ session: "web3", lines: [{ offer: (s) => storeMint(s), name: "Rainbow Hat" }], walletAccount: BOOK.reseller, checkout: seeded("failed") });
    await screen.findByText("Your wallet switched accounts");
    expect(screen.getByTestId("resume-checkout")).toBeDisabled();
    await act(async () => fireEvent.click(screen.getByTestId("resume-checkout")));
    expect(demo.shop.log.some((r) => r.name === "authorizeCreditBatch")).toBe(false);
    expect(demo.relay.posts).toHaveLength(0);
  });

  it("confirming over a group left authorizing cancels its reserved credit before authorizing a new one", async () => {
    const { demo } = renderCart({ session: "web3", lines: [{ offer: (s) => storeMint(s), name: "Rainbow Hat" }], checkout: seeded("authorizing", ["0xold"]) });
    await clickWhenEnabled(confirmButton);
    await screen.findByText("SUCCESS PAGE", {}, { timeout: 5000 });
    const names = demo.shop.log.map((r) => r.name);
    const cancel = demo.shop.log.find((r) => r.name === "cancelCreditAuthorizations");
    expect((cancel?.body as { salts: string[] } | undefined)?.salts).toEqual(["0xold"]);
    expect(names.indexOf("cancelCreditAuthorizations")).toBeLessThan(names.indexOf("authorizeCreditBatch"));
  });

  it("account change mid-flow: a wallet on another account blocks confirming until it is resolved", async () => {
    renderCart({ session: "web3", lines: MIXED, walletAccount: BOOK.reseller });
    await screen.findByText("Your wallet switched accounts");
    expect(confirmButton()).toBeDisabled();
  });

  it("LAND in a web3 cart is MANA only on Ethereum, in its own group", async () => {
    renderCart({ session: "web3", lines: [{ offer: (s) => parcelResale(s), listing: { orderId: "order-1" }, name: "Parcel 12,-4" }] });
    expect(screen.getByTestId("payment-mana-only").textContent).toContain("Paid in MANA on Ethereum");
    expect(screen.getByTestId("payment-mana-only").textContent).toContain("LAND, estates and rentals are sold in MANA.");
    expect(screen.getByTestId("total-mana-1").textContent).toMatch(/4,000\s*MANA/);
    expect(screen.queryByTestId("total-credits")).toBeNull();
    await walletSettled();
  });
});
