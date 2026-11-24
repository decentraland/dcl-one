import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { MemoryRouter, createRoutesStub, useLocation, useNavigate } from "react-router";
import { afterEach, describe, expect, it, onTestFinished, vi } from "vitest";

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
let here = "";
function Navigator() {
  const navigate = useNavigate();
  here = useLocation().pathname;
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

/** Stands in for the browser's modal dialog methods, which jsdom lacks, until the test ends. */
function nativeDialog(methods: Pick<HTMLDialogElement, "showModal" | "close">) {
  const proto = HTMLDialogElement.prototype;
  const before = { showModal: Object.getOwnPropertyDescriptor(proto, "showModal"), close: Object.getOwnPropertyDescriptor(proto, "close") };
  Object.defineProperty(proto, "showModal", { configurable: true, value: methods.showModal });
  Object.defineProperty(proto, "close", { configurable: true, value: methods.close });
  onTestFinished(() => {
    for (const name of ["showModal", "close"] as const) {
      const descriptor = before[name];
      if (descriptor) Object.defineProperty(proto, name, descriptor);
      else Reflect.deleteProperty(proto, name);
    }
  });
}

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

  it("is a native modal dialog, closed by its own close() before it leaves the document, so the browser gives focus back", () => {
    // jsdom has no modal dialogs: these stand in for the browser's, which move focus in, keep it inside and give it
    // back on close (Esc, focus and scroll: CartDrawer.browser.test.ts).
    const calls: string[] = [];
    nativeDialog({
      showModal(this: HTMLDialogElement) {
        calls.push("showModal");
        this.setAttribute("open", "");
      },
      close(this: HTMLDialogElement) {
        calls.push(this.isConnected ? "close" : "close outside the document");
        expect(within(this).getByRole("button", { name: "Continue shopping" })).toBeTruthy();
        this.removeAttribute("open");
      },
    });
    const opener = document.body.appendChild(document.createElement("button"));
    onTestFinished(() => opener.remove());
    opener.focus();
    const focus = vi.spyOn(opener, "focus");
    const { cart, drawer } = setup();
    act(() => {
      cart.add(hat);
      drawer.added();
    });
    expect(calls).toEqual(["showModal"]);
    fireEvent.click(within(dialog()).getByRole("button", { name: "Continue shopping" }));
    expect(calls).toEqual(["showModal", "close"]);
    expect(screen.queryByTestId("cart-drawer")).toBeNull();
    expect(drawer.getSnapshot().open).toBe(false);
    expect(focus).not.toHaveBeenCalled();
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

  it("Go to cart only navigates: the dialog closes on the cart page, so the click does not wait for the close", () => {
    const closedOn: string[] = [];
    nativeDialog({
      showModal(this: HTMLDialogElement) {
        this.setAttribute("open", "");
      },
      close(this: HTMLDialogElement) {
        closedOn.push(here);
        this.removeAttribute("open");
      },
    });
    const { cart, drawer } = setup();
    act(() => {
      cart.add(hat);
      drawer.added();
    });
    fireEvent.click(within(dialog()).getByRole("link", { name: "Go to cart" }));
    expect(closedOn).toEqual(["/shop/cart"]);
    expect(screen.queryByTestId("cart-drawer")).toBeNull();
    expect(drawer.getSnapshot().open).toBe(false);
  });

  it("on the cart page, where Go to cart changes no page, the click closes it", () => {
    const { cart, drawer } = setup();
    act(() => go!("/shop/cart"));
    act(() => {
      cart.add(hat);
      drawer.added();
    });
    fireEvent.click(within(dialog()).getByRole("link", { name: "Go to cart" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(drawer.getSnapshot().open).toBe(false);
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

  it("Continue shopping, the scrim's outside click and Escape (the dialog's cancel) close it", () => {
    const { cart, drawer } = setup();
    act(() => {
      cart.add(hat);
      drawer.added();
    });
    fireEvent.click(within(dialog()).getByRole("button", { name: "Continue shopping" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    act(() => drawer.open());
    fireEvent.click(dialog().firstElementChild!);
    expect(screen.queryByRole("dialog")).toBeNull();
    act(() => drawer.open());
    fireEvent(dialog(), new Event("cancel"));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(drawer.getSnapshot().open).toBe(false);
  });

  it("follows a close the browser makes itself, unless the drawer is open again by then", () => {
    const { cart, drawer } = setup();
    act(() => {
      cart.add(hat);
      drawer.added();
    });
    const d = screen.getByTestId("cart-drawer");
    fireEvent(d, new Event("close"));
    expect(drawer.getSnapshot().open).toBe(true);
    d.removeAttribute("open");
    fireEvent(d, new Event("close"));
    expect(drawer.getSnapshot().open).toBe(false);
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

describe("Go to cart's preload", () => {
  const preload = () => document.querySelector('link[rel="modulepreload"]');
  // jsdom runs no CSS animations: each test says what the panel's getAnimations() answers.
  const animations = (list: Animation[]) =>
    Object.defineProperty(HTMLElement.prototype, "getAnimations", { configurable: true, value: () => list });
  afterEach(() => void Reflect.deleteProperty(HTMLElement.prototype, "getAnimations"));

  function framework() {
    const cart = createCartStore({ storage: () => memory(), now: () => 1 });
    const drawer = createCartDrawer();
    const Stub = createRoutesStub([
      { path: "/shop/items", Component: () => <CartDrawer cart={cart} drawer={drawer} /> },
      { path: "/shop/cart", Component: () => null },
    ]);
    render(<Stub initialEntries={["/shop/items"]} />);
    act(() => void cart.add(hat));
    return drawer;
  }

  /** The panel's slide-in as the browser runs it: an animation that finishes when `end` is called. */
  function slide() {
    let end: () => void = () => {};
    const finished = new Promise<void>((resolve) => {
      end = () => resolve();
    });
    animations([{ finished } as unknown as Animation]);
    return () => act(async () => end());
  }

  it("preloads the cart page's code once the panel has slid in, not while it slides or while the drawer is closed", async () => {
    const end = slide();
    const drawer = framework();
    expect(preload()).toBeNull();
    act(() => drawer.added());
    expect(screen.getByRole("link", { name: "Go to cart" })).toBeTruthy();
    expect(preload()).toBeNull();
    await end();
    expect(preload()).not.toBeNull();
  });

  it("preloads at once when the panel does not slide (reduced motion), and again after a close and an open", async () => {
    animations([]);
    const drawer = framework();
    await act(async () => drawer.added());
    expect(preload()).not.toBeNull();
    act(() => drawer.close());
    expect(preload()).toBeNull();
    await act(async () => drawer.open());
    expect(preload()).not.toBeNull();
  });

  it("is nothing in the in-world HUD, whose router has no preload: Go to cart stays a plain link", () => {
    const { cart, drawer } = setup();
    act(() => {
      cart.add(hat);
      drawer.added();
    });
    expect(preload()).toBeNull();
    expect(within(dialog()).getByRole("link", { name: "Go to cart" }).getAttribute("href")).toBe("/shop/cart");
  });
});
