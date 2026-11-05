
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useLocation } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { shopCart } from "../../cart/store";
import { itemAsset } from "../../model/asset";
import { evmAddress } from "../../model/address";
import { ShopHarness, type SessionKind } from "../fixtures";
import ShopChrome, { shopTabs } from "./ShopChrome";

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

  it("hides the credits entry in the iOS web view", async () => {
    window.history.pushState({}, "", "/?view=mobile-iap");
    await chrome("/shop/overview");
    expect(screen.queryByRole("link", { name: /buy credits/i })).toBeNull();
  });
});
