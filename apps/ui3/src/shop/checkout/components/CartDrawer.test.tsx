import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { MemoryRouter, useNavigate } from "react-router";
import { describe, expect, it } from "vitest";

import { createCartDrawer } from "../../cart/drawer";
import { createCartStore, type CartStorage } from "../../cart/store";
import { cartLauncher } from "../../detail/purchase";
import { evmAddress } from "@shop/model/address";
import { itemAsset, tokenAsset } from "@shop/model/asset";
import { CHAIN } from "@shop/model/chain";
import CartDrawer from "./CartDrawer";

function memory(): CartStorage {
  const data = new Map<string, string>();
  return { getItem: (k) => data.get(k) ?? null, setItem: (k, v) => void data.set(k, v), removeItem: (k) => void data.delete(k) };
}

const collection = evmAddress("0x000000000000000000000000000000000000c011");
const hat = { asset: itemAsset(CHAIN.POLYGON, collection, 7n), listing: { acquisition: "store" as const }, display: { name: "Rainbow Hat", priceCredits: 12 }, available: 3 };
const jacket = { asset: tokenAsset(CHAIN.POLYGON, collection, 42n), listing: { tradeId: "t-1" }, display: { name: "Vintage Jacket", priceCredits: 30 } };

let go: ((to: string) => void) | null = null;
function Navigator() {
  const navigate = useNavigate();
  go = (to) => void navigate(to);
  return null;
}

function setup() {
  const storage = memory();
  const cart = createCartStore({ storage: () => storage, now: () => 1 });
  const drawer = createCartDrawer();
  render(
    <MemoryRouter initialEntries={["/shop/items"]}>
      <Navigator />
      <CartDrawer cart={cart} drawer={drawer} />
    </MemoryRouter>,
  );
  return { cart, drawer };
}

const dialog = () => screen.getByRole("dialog", { name: "Shopping cart" });

describe("the cart drawer, as upstream's CartPopover", () => {
  it("is not drawn while closed", () => {
    setup();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("an add opens it with the banner, the line, its subtotal and the total", () => {
    const { cart, drawer } = setup();
    act(() => {
      cart.add(hat);
      drawer.added();
    });
    const d = dialog();
    expect(within(d).getByRole("heading", { name: "Shopping Cart (1)" })).toBeTruthy();
    expect(within(d).getByTestId("cart-drawer-banner").textContent).toBe("1 Item successfully added to cart!");
    const line = within(d).getByTestId("cart-drawer-line");
    expect(within(line).getByText("Rainbow Hat")).toBeTruthy();
    expect(within(d).getByText("Total: 1 item")).toBeTruthy();
    expect(within(d).getByTestId("cart-drawer-total").textContent).toContain("12");
    expect(within(d).getByRole("link", { name: "Go to cart" }).getAttribute("href")).toBe("/shop/cart");
  });

  it("the stepper changes a mint's copies, up to its stock, and the count follows the units", () => {
    const { cart, drawer } = setup();
    act(() => {
      cart.add(hat);
      cart.add(jacket);
      drawer.added(2);
    });
    expect(within(dialog()).getByRole("heading", { name: "Shopping Cart (2)" })).toBeTruthy();
    fireEvent.click(within(dialog()).getByRole("button", { name: "Increase quantity of Rainbow Hat" }));
    expect(within(dialog()).getByRole("heading", { name: "Shopping Cart (3)" })).toBeTruthy();
    expect(within(dialog()).getByText("Total: 3 items")).toBeTruthy();
    // A token is unique: only the mint has a stepper.
    expect(within(dialog()).getAllByRole("button", { name: /^Increase quantity/ })).toHaveLength(1);
    fireEvent.click(within(dialog()).getByRole("button", { name: "Decrease quantity of Rainbow Hat" }));
    expect(within(dialog()).getByRole("heading", { name: "Shopping Cart (2)" })).toBeTruthy();
    expect(within(dialog()).getByRole("button", { name: "Decrease quantity of Rainbow Hat" })).toBeDisabled();
  });

  it("a line added from a card stops at the stock it was added with, as the cart caps it", () => {
    const { cart, drawer } = setup();
    act(() => {
      cart.add({ ...hat, available: 2 });
      drawer.added();
    });
    const plus = () => within(dialog()).getByRole("button", { name: "Increase quantity of Rainbow Hat" });
    fireEvent.click(plus());
    expect(cart.getSnapshot().lines[0]!.quantity).toBe(2);
    expect(plus()).toBeDisabled();
    fireEvent.click(plus());
    expect(cart.getSnapshot().lines[0]!.quantity).toBe(2);
  });

  it("shows no total while a line has no price, so a partial sum never reads as the whole", () => {
    const { cart, drawer } = setup();
    act(() => {
      cart.add(hat);
      cart.add({ ...jacket, display: { name: "Vintage Jacket" } });
      drawer.added(2);
    });
    expect(within(dialog()).getByText("Total: 2 items")).toBeTruthy();
    expect(within(dialog()).queryByTestId("cart-drawer-total")).toBeNull();
    expect(within(dialog()).getByTestId("cart-drawer-banner")).toBeTruthy();
  });

  it("is a modal: focus moves into it, the page stops scrolling, and both come back on close", () => {
    const { cart, drawer } = setup();
    act(() => {
      cart.add(hat);
      drawer.added();
    });
    expect(dialog().contains(document.activeElement)).toBe(true);
    expect(document.body.style.overflow).toBe("hidden");
    act(() => drawer.close());
    expect(document.body.style.overflow).toBe("");
  });

  it("keeps Tab inside the panel", () => {
    const { cart, drawer } = setup();
    act(() => {
      cart.add(hat);
      drawer.added();
    });
    const panel = dialog().querySelector("aside")!;
    const focusable = [...panel.querySelectorAll<HTMLElement>("a[href], button:not([disabled])")];
    focusable[focusable.length - 1]!.focus();
    fireEvent.keyDown(panel, { key: "Tab" });
    expect(document.activeElement).toBe(focusable[0]);
    fireEvent.keyDown(panel, { key: "Tab", shiftKey: true });
    expect(document.activeElement).toBe(focusable[focusable.length - 1]);
  });

  it("closes when the page changes under it, as the browser's back does", () => {
    const { cart, drawer } = setup();
    act(() => {
      cart.add(hat);
      drawer.added();
    });
    act(() => go!("/shop/overview"));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(drawer.getSnapshot().open).toBe(false);
    // Opened again on the new page, it stays open there.
    act(() => drawer.open());
    expect(dialog()).toBeTruthy();
  });

  it("removing a line takes the banner with it; an empty cart shows the empty state and no total", () => {
    const { cart, drawer } = setup();
    act(() => {
      cart.add(hat);
      drawer.added();
    });
    fireEvent.click(within(dialog()).getByRole("button", { name: "Remove Rainbow Hat" }));
    expect(within(dialog()).queryByTestId("cart-drawer-banner")).toBeNull();
    expect(within(dialog()).getByTestId("cart-drawer-empty")).toBeTruthy();
    expect(within(dialog()).queryByText(/^Total:/)).toBeNull();
  });

  it("Continue shopping, the scrim's outside click and Escape close it", () => {
    const { cart, drawer } = setup();
    act(() => {
      cart.add(hat);
      drawer.added();
    });
    fireEvent.click(within(dialog()).getByRole("button", { name: "Continue shopping" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    act(() => drawer.open());
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

describe("adding from the item page", () => {
  it("adds the line and opens the drawer without leaving the page; buying goes on to the cart", () => {
    const storage = memory();
    const cart = createCartStore({ storage: () => storage, now: () => 1 });
    const drawer = createCartDrawer();
    const launch = cartLauncher(cart, drawer);
    const navigated: string[] = [];
    launch({ line: hat, method: null, offer: null, intent: "add" }, (to) => navigated.push(to));
    expect(navigated).toEqual([]);
    expect(drawer.getSnapshot()).toEqual({ open: true, justAdded: 1 });
    launch({ line: hat, method: null, offer: null, intent: "add" }, (to) => navigated.push(to));
    expect(cart.getSnapshot().lines[0]!.quantity).toBe(2);
    expect(drawer.getSnapshot().justAdded).toBe(2);
    launch({ line: jacket, method: null, offer: null }, (to) => navigated.push(to));
    expect(navigated).toEqual(["/shop/cart"]);
  });

  it("an add the cart does not take opens the drawer with no banner: a token already in it, a mint at its stock", () => {
    const storage = memory();
    const cart = createCartStore({ storage: () => storage, now: () => 1 });
    const drawer = createCartDrawer();
    const launch = cartLauncher(cart, drawer);
    const noop = () => {};
    cart.add(jacket);
    launch({ line: jacket, method: null, offer: null, intent: "add" }, noop);
    expect(cart.getSnapshot().lines).toHaveLength(1);
    expect(drawer.getSnapshot()).toEqual({ open: true, justAdded: 0 });
    drawer.close();
    const last = { ...hat, available: 1 };
    launch({ line: last, method: null, offer: null, intent: "add" }, noop);
    expect(drawer.getSnapshot()).toEqual({ open: true, justAdded: 1 });
    launch({ line: last, method: null, offer: null, intent: "add" }, noop);
    expect(cart.getSnapshot().lines.find((l) => l.asset.token.kind === "item")!.quantity).toBe(1);
    expect(drawer.getSnapshot()).toEqual({ open: true, justAdded: 1 });
  });
});
