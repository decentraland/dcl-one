
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useLocation } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { createShopClient, type ShopClient } from "../../api/shop";
import { shopCart } from "../../cart/store";
import { itemAsset } from "../../model/asset";
import { evmAddress } from "../../model/address";
import { catalogTransport, ShopHarness, type SessionKind } from "../fixtures";
import ShopChrome, { shopTabs } from "./ShopChrome";
import { formatCreditsBalance } from "./ShopCreditsChip";

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn(async () => new Response("{}", { status: 200 })));
  shopCart.clear();
});
afterEach(() => {
  vi.unstubAllGlobals();
  window.history.pushState({}, "", "/");
});

function Where() {
  const { pathname } = useLocation();
  return <output data-testid="where">{pathname}</output>;
}

async function chrome(entry: string, session: SessionKind = "signedOut", client?: ShopClient) {
  render(
    <ShopHarness path="/shop/*" entry={entry} session={session} client={client}>
      <ShopChrome footer={false}>
        <Where />
      </ShopChrome>
    </ShopHarness>,
  );
  await act(async () => {});
}

describe("the Shop frame", () => {
  it("lists the unified Shop's sections, the event only when one runs, My Items not in the iOS web view", () => {
    expect(shopTabs({ iap: false, eventLabel: null }).map((t) => t.id)).toEqual(["overview", "collectibles", "my-items"]);
    expect(shopTabs({ iap: true, eventLabel: "Halloween" }).map((t) => t.id)).toEqual(["overview", "event", "collectibles"]);
  });

  it("renders the subnav; the cart shows its count and activity needs a session", async () => {
    shopCart.add({ asset: itemAsset(137, evmAddress("0x000000000000000000000000000000000000c011"), 7), display: { name: "Nacho Hat" }, quantity: 2 });
    await chrome("/shop/items/creator/0xabc", "web2");
    const subnav = screen.getAllByTestId("subnav")[0]!;
    expect(within(subnav).getByTestId("subnav-cart-badge")).toHaveTextContent("2");
    expect(within(subnav).getByTestId("subnav-activity")).toBeTruthy();
    expect(within(subnav).getByRole("link", { name: /buy credits/i }).getAttribute("href")).toBe("/shop/credits");
    expect(within(subnav).getByTestId("search")).toBeTruthy();
  });

  it("navigates between sections through the host's router", async () => {
    await chrome("/shop/overview");
    fireEvent.click(screen.getAllByRole("link", { name: "Collectibles" })[0]!);
    await waitFor(() => expect(screen.getByTestId("where")).toHaveTextContent("/shop/items"));
  });

  it("marks the cart page only through the cart button", async () => {
    await chrome("/shop/cart");
    expect(within(screen.getByTestId("subnav-tabs")).queryByRole("link", { name: "Cart" })).toBeNull();
    expect(screen.getByTestId("subnav-cart")).toBeTruthy();
  });

  it("shows a signed-in buyer's credits in the global bar, before the bell, as a way to the pack picker", async () => {
    const balance = { credits: [], totalCredits: 0, totals: { expiring: 0, nonExpiring: 0 }, usd: { balanceCents: 123_450, credits: 12_345 } };
    const transport = catalogTransport("ok", { "/v1/users/0x0000000000000000000000000000000000000b0b/credits": balance });
    const signer = { address: "0x0000000000000000000000000000000000000001", sign: async () => ({}) };
    await chrome("/shop/items", "web2", createShopClient({ base: "http://shop.test", mode: "canonical", transport, signer: async () => signer }));
    const chip = await screen.findByTestId("navbar-credits");
    expect(chip).toHaveTextContent("12,345");
    expect(chip.getAttribute("aria-label")).toBe("12,345 shop credits");
    expect(chip.getAttribute("href")).toBe("/shop/credits");
    expect(chip.compareDocumentPosition(screen.getByRole("button", { name: "Notifications" })) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("puts credits a purchase holds beside Buy Credits, and nothing there when none are held", async () => {
    const held = { cents: 90, credits: 9, releasesAtSeconds: null, purchases: [] };
    const balance = { credits: [], totalCredits: 0, totals: { expiring: 0, nonExpiring: 0 }, usd: { balanceCents: 1_000, credits: 100, held } };
    const transport = catalogTransport("ok", { "/v1/users/0x0000000000000000000000000000000000000b0b/credits": balance });
    const signer = { address: "0x0000000000000000000000000000000000000001", sign: async () => ({}) };
    await chrome("/shop/items", "web2", createShopClient({ base: "http://shop.test", mode: "canonical", transport, signer: async () => signer }));
    const note = await screen.findByTestId("held-credits");
    expect(note.textContent).toContain("9");
    expect(note.compareDocumentPosition(screen.getAllByRole("link", { name: /buy credits/i })[0]!) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("shows no credits chip signed out", async () => {
    await chrome("/shop/items");
    expect(screen.queryByTestId("navbar-credits")).toBeNull();
  });

  it("writes the balance as ui2 does: whole credits, grouped", () => {
    expect(formatCreditsBalance(0)).toBe("0");
    expect(formatCreditsBalance(1234.9)).toBe("1,234");
  });

  it("hides the credits entry in the iOS web view", async () => {
    window.history.pushState({}, "", "/?view=mobile-iap");
    await chrome("/shop/overview");
    expect(screen.queryByRole("link", { name: /buy credits/i })).toBeNull();
  });
});
