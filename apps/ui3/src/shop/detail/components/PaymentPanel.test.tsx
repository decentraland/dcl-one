
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { describe, expect, it, vi } from "vitest";

import { manaFromWhole, usdCents, type ManaWei, type UsdCents } from "../../model/money";
import type { PurchaseOffer } from "../../model/purchase";
import type { Session } from "../../model/settlement";
import { nameRegistration, parcelResale, SIGNED_OUT, storeMint, wearableResale, web2, web3, BOOK } from "../../payment/fixtures";
import { ruleFor } from "../../payment/rule";
import { withCreditsOutage } from "../fixtures";
import type { OffersStatus } from "../offers";
import type { GuidanceRevalidation } from "../../wallet/ManaGuidance";
import PaymentPanel, { type ListingPrice, type WalletView } from "./PaymentPanel";

const WALLET: WalletView = { account: BOOK.buyer, walletAccount: BOOK.buyer, chainId: 137, pendingChainId: null, accountMismatch: false };

function show(
  offer: PurchaseOffer | null,
  session: Session,
  opts: {
    status?: OffersStatus;
    listing?: ListingPrice | null;
    wallet?: Partial<WalletView>;
    creditsBalance?: UsdCents | null;
    manaBalance?: ManaWei | null;
    setupUrl?: string;
    revalidation?: GuidanceRevalidation | null;
  } = {},
) {
  const onBuy = vi.fn();
  const onAddToCart = vi.fn();
  const onRetry = vi.fn();
  render(
    <MemoryRouter>
      <PaymentPanel
        offer={offer}
        status={opts.status ?? (offer ? "ready" : "routeMissing")}
        listing={opts.listing ?? null}
        session={session}
        wallet={{ ...WALLET, ...opts.wallet }}
        item={{ name: "Nacho Hat", href: "/shop/item/0x1/7" }}
        creditsBalance={opts.creditsBalance}
        manaBalance={opts.manaBalance}
        setupUrl={opts.setupUrl}
        revalidation={opts.revalidation}
        onRetry={onRetry}
        onBuy={onBuy}
        onAddToCart={onAddToCart}
      />
    </MemoryRouter>,
  );
  return { onBuy, onAddToCart, onRetry };
}

describe("an eligible primary wearable (a store mint)", () => {
  it("web3: credits first and selected, MANA second in exact units, buy with credits", () => {
    const { onBuy } = show(storeMint(web3()), web3());
    const credits = screen.getByTestId("pay-credits") as HTMLInputElement;
    const mana = screen.getByTestId("pay-mana") as HTMLInputElement;
    expect(credits.checked).toBe(true);
    expect(mana.checked).toBe(false);
    expect(screen.getByTestId("price-credits").textContent).toContain("12");
    expect(screen.getByTestId("price-mana").textContent).toContain("2.3");
    fireEvent.click(screen.getByTestId("cta-buy"));
    expect(onBuy).toHaveBeenCalledWith("credits", expect.anything(), undefined);
  });

  it("web3: choosing MANA buys with MANA and names its network", () => {
    const { onBuy } = show(storeMint(web3()), web3());
    fireEvent.click(screen.getByTestId("pay-mana"));
    expect(screen.getByTestId("mana-network").textContent).toContain("Polygon");
    fireEvent.click(screen.getByTestId("cta-buy"));
    expect(onBuy).toHaveBeenCalledWith("mana", expect.anything(), undefined);
  });

  it("web3 with some credits: the credits+MANA split is an opt-in inside the MANA choice only, and it is what Buy runs", () => {
    const { onBuy } = show(storeMint(web3()), web3(), { creditsBalance: usdCents(50) });
    expect(screen.queryByTestId("mixed-payment")).toBeNull();
    fireEvent.click(screen.getByTestId("pay-mana"));
    const split = screen.getByTestId("mixed-payment");
    expect(split.textContent).toBe("Use 5 Credits from your balance first and pay the other 1.3 MANA from your wallet.");
    expect(screen.getByTestId("pay-mixed")).not.toBeChecked();
    fireEvent.click(screen.getByTestId("cta-buy"));
    expect(onBuy).toHaveBeenLastCalledWith("mana", expect.anything(), undefined);
    fireEvent.click(screen.getByTestId("pay-mixed"));
    expect(screen.getByTestId("cta-buy").querySelector("span")!.textContent).toBe("Buy with Credits and MANA");
    fireEvent.click(screen.getByTestId("cta-buy"));
    expect(onBuy).toHaveBeenLastCalledWith("mana", expect.anything(), expect.objectContaining({ credits: 5, creditsCents: 50 }));
  });

  it("the split is not offered when the wallet's MANA cannot cover the rest", () => {
    show(storeMint(web3()), web3(), { creditsBalance: usdCents(50), manaBalance: manaFromWhole(1) as ManaWei });
    fireEvent.click(screen.getByTestId("pay-mana"));
    expect(screen.queryByTestId("mixed-payment")).toBeNull();
  });

  it("web2: credits only, no MANA option", () => {
    show(storeMint(web2()), web2());
    expect(screen.queryByTestId("pay-mana")).toBeNull();
    expect(screen.getByTestId("price-credits").textContent).toContain("12");
    expect(screen.getByTestId("cta-buy").querySelector("span")!.textContent).toBe("Buy with Credits");
  });

  it("signed out: credits, and the action is to sign in", () => {
    const signIn = vi.fn();
    show(storeMint(SIGNED_OUT), SIGNED_OUT, { wallet: { onSignIn: signIn } });
    expect(screen.queryByTestId("pay-mana")).toBeNull();
    fireEvent.click(screen.getByTestId("cta-sign-in"));
    expect(signIn).toHaveBeenCalled();
  });

  it("short credits: an explicit top-up, credits stay selected, nothing moves to MANA", () => {
    show(storeMint(web3()), web3(), { creditsBalance: usdCents(30) });
    expect((screen.getByTestId("pay-credits") as HTMLInputElement).checked).toBe(true);
    expect(screen.getByTestId("state-topUpRequired").textContent).toMatch(/Top up/);
    expect(screen.getByTestId("cta-top-up").getAttribute("href")).toBe("/shop/credits?returnTo=%2Fshop%2Fitem%2F0x1%2F7");
    expect(screen.queryByTestId("cta-buy")).toBeNull();
  });

  it("a credits outage: credits stay the main choice, unavailable with the reason and a retry; MANA stays second", () => {
    const { onRetry } = show(withCreditsOutage(storeMint(web3())), web3());
    expect((screen.getByTestId("pay-credits") as HTMLInputElement).checked).toBe(true);
    expect(screen.getByTestId("payment-outage").textContent).toMatch(/Credits price is unavailable right now: the MANA price feed/);
    expect((screen.getByTestId("cta-buy") as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByTestId("pay-mana")).toBeTruthy();
    fireEvent.click(screen.getByText("Retry"));
    expect(onRetry).toHaveBeenCalled();
  });

  it("a credits outage for web2 offers no MANA in its place", () => {
    show(withCreditsOutage(storeMint(web2())), web2());
    expect(screen.queryByTestId("pay-mana")).toBeNull();
    expect(screen.getByTestId("payment-outage")).toBeTruthy();
  });
});

describe("a MANA-only purchase", () => {
  it("web3 resale: MANA only, no credit price, no top-up, no credits+MANA", () => {
    show(wearableResale(web3()), web3(), { creditsBalance: usdCents(5) });
    const panel = screen.getByTestId("payment-panel");
    expect(panel.dataset.manaOnly).toBeDefined();
    expect(screen.queryByTestId("pay-credits")).toBeNull();
    expect(panel.textContent).not.toMatch(/Credit/);
    expect(screen.getByTestId("mana-only-reasons").textContent).toContain("This is a resale");
    expect(screen.getByTestId("cta-buy").querySelector("span")!.textContent).toBe("Buy with MANA");
  });

  it("web2 resale: the network, exact MANA and wallet guidance, never credits", () => {
    show(wearableResale(web2()), web2());
    const panel = screen.getByTestId("payment-panel");
    expect(panel.textContent).toContain("This purchase is paid in MANA");
    expect(panel.textContent).toContain("Polygon");
    expect(panel.textContent).not.toMatch(/Credit/);
    expect((screen.getByTestId("cta-buy") as HTMLButtonElement).disabled).toBe(true);
  });

  it("web2 resale: the guidance carries the wallet setup path and the after-switch check", () => {
    const onRetry = vi.fn();
    show(wearableResale(web2()), web2(), { wallet: { walletAccount: null }, setupUrl: "https://wallets.test/setup", revalidation: { status: "failed", onRetry } });
    expect(screen.getByRole("link", { name: "Set up a wallet" })).toHaveAttribute("href", "https://wallets.test/setup");
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(onRetry).toHaveBeenCalled();
  });

  it("web2 LAND on Ethereum: MANA on Ethereum with gas, and no credit wording", () => {
    show(parcelResale(web2()), web2());
    const panel = screen.getByTestId("payment-panel");
    expect(panel.textContent).toContain("LAND, estates and rentals are sold in MANA");
    expect(panel.textContent).toContain("Ethereum");
    expect(panel.textContent).not.toMatch(/Credit/);
  });

  it("a wallet on another account than the session: explicit switch before a MANA purchase", () => {
    const switchAccount = vi.fn();
    show(wearableResale(web3()), web3(), { wallet: { accountMismatch: true, walletAccount: BOOK.reseller, onSwitchAccount: switchAccount } });
    expect((screen.getByTestId("cta-buy") as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByText("Switch account"));
    expect(switchAccount).toHaveBeenCalled();
  });
});

describe("NAME registration", () => {
  it("web3: credits first, the registrar's MANA second", () => {
    show(nameRegistration(web3()), web3());
    expect((screen.getByTestId("pay-credits") as HTMLInputElement).checked).toBe(true);
    expect(screen.getByTestId("price-mana").textContent).toContain("100");
  });
});

describe("before the server answers", () => {
  const listing = (session: Session): ListingPrice => ({
    eligibility: ruleFor({ asset: storeMint(session).asset, kind: "wearable", sale: "primaryMint", bundle: false, settlement: [] }, session),
    credits: 12,
    mana: null,
    chainId: 137,
  });

  it("loading: a skeleton, no price committed", () => {
    show(null, web3(), { status: "loading", listing: listing(web3()) });
    expect(screen.getByTestId("payment-loading").getAttribute("aria-busy")).toBe("true");
  });

  it("no quote route: the rule's choices from the listing, unavailable, with the explanation and a retry", () => {
    show(null, web3(), { status: "routeMissing", listing: listing(web3()) });
    expect((screen.getByTestId("pay-credits") as HTMLInputElement).checked).toBe(true);
    expect(screen.getByTestId("pay-mana")).toBeTruthy();
    expect(screen.getByTestId("price-credits").textContent).toContain("12");
    expect(screen.getByTestId("payment-outage").textContent).toContain("can't be confirmed");
    expect((screen.getByTestId("cta-buy") as HTMLButtonElement).disabled).toBe(true);
  });

  it("signed out with a failed quote: the listing's credit price and an active Buy now that signs in, no error yet", () => {
    const signIn = vi.fn();
    show(null, SIGNED_OUT, { status: "failed", listing: listing(SIGNED_OUT), wallet: { onSignIn: signIn } });
    expect(screen.getByTestId("item-price").textContent).toContain("12");
    expect(screen.queryByTestId("payment-outage")).toBeNull();
    expect(screen.queryByTestId("pay-mana")).toBeNull();
    const cta = screen.getByTestId("cta-sign-in") as HTMLButtonElement;
    expect(cta.disabled).toBe(false);
    expect(cta.textContent).toMatch(/Buy now/i);
    fireEvent.click(cta);
    expect(signIn).toHaveBeenCalled();
  });

  it("a failed quote for a resale keeps it MANA only with the guidance", () => {
    const session = web2();
    const resale = wearableResale(session);
    const eligibility = ruleFor(resale, session);
    show(null, session, { status: "failed", listing: { eligibility, credits: null, mana: manaFromWhole(2), chainId: 137 } });
    const panel = screen.getByTestId("payment-panel");
    expect(panel.textContent).toContain("could not be loaded");
    expect(panel.textContent).toContain("This purchase is paid in MANA");
    expect(panel.textContent).not.toMatch(/Credit/);
  });
});
