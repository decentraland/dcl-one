
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useLocation } from "react-router";
import { afterEach, beforeEach, describe, expect, it, onTestFinished, vi } from "vitest";

import { shopCartDrawer } from "../../cart/drawer";
import { shopCart } from "../../cart/store";
import { itemAsset } from "../../model/asset";
import { evmAddress } from "../../model/address";
import { marketplaceOn } from "../../market/testing-flags";
import { ShopHarness, type SessionKind } from "../fixtures";
import ShopChrome, { shopTabs } from "./ShopChrome";

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn(async () => new Response("{}", { status: 200 })));
  shopCart.clear();
  shopCartDrawer.close();
});
afterEach(() => {
  vi.unstubAllGlobals();
  window.history.pushState({}, "", "/");
});

function Where() {
  const { pathname } = useLocation();
  return <output data-testid="where">{pathname}</output>;
}

async function chrome(entry: string, session: SessionKind = "signedOut") {
  render(
    <ShopHarness path="/shop/*" entry={entry} session={session}>
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

  it("the subnav's cart button opens the drawer, with no banner, and stays on the page", async () => {
    await chrome("/shop/items");
    fireEvent.click(screen.getAllByTestId("subnav-cart")[0]!);
    expect(screen.getByRole("dialog", { name: "Shopping cart" })).toBeTruthy();
    expect(screen.queryByTestId("cart-drawer-banner")).toBeNull();
    expect(screen.getByTestId("where")).toHaveTextContent("/shop/items");
  });

  it("mounts the drawer on a marketplace page too, where a card's Add to cart still opens it", async () => {
    await chrome("/shop/accounts/0x000000000000000000000000000000000000abcd");
    act(() => shopCartDrawer.added());
    expect(screen.getByRole("dialog", { name: "Shopping cart" })).toBeTruthy();
  });

  it("hides the credits entry in the iOS web view", async () => {
    window.history.pushState({}, "", "/?view=mobile-iap");
    await chrome("/shop/overview");
    expect(screen.queryByRole("link", { name: /buy credits/i })).toBeNull();
  });

  it("shows the Marketplace's tabs on its pages only to a visitor who has it; Settings is the Shop's", async () => {
    await chrome("/shop/accounts/0x1", "web2");
    expect(within(screen.getAllByTestId("subnav")[0]!).queryByRole("link", { name: "Land" })).toBeNull();
    document.body.innerHTML = "";
    onTestFinished(marketplaceOn());
    await chrome("/shop/accounts/0x1", "web3");
    expect(within(screen.getAllByTestId("subnav")[0]!).getAllByRole("link", { name: "Land" }).length).toBeGreaterThan(0);
    document.body.innerHTML = "";
    await chrome("/shop/settings", "web3");
    expect(screen.getAllByTestId("subnav")[0]!).toHaveAttribute("data-set", "shop");
  });
});
