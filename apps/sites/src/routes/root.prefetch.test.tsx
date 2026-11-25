// @vitest-environment jsdom
// Go to cart in the Shop's cart drawer preloads the cart page once the panel has slid in. With root's own
// shouldRevalidate, root's data stays between Shop pages, so the preload holds the page's code and no data request.
import { act, cleanup, render, screen } from "@testing-library/react";
import { createRoutesStub, Outlet, type ShouldRevalidateFunction } from "react-router";
import { afterEach, expect, it, vi } from "vitest";

import { evmAddress } from "@shop/model/address";
import { itemAsset } from "@shop/model/asset";
import { CHAIN } from "@shop/model/chain";
import { createCartDrawer } from "@ui/shop/cart/drawer";
import { createCartStore } from "@ui/shop/cart/store";
import CartDrawer from "@ui/shop/checkout/components/CartDrawer";

import { shouldRevalidate } from "./root";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
});

const hat = { asset: itemAsset(CHAIN.POLYGON, evmAddress("0x000000000000000000000000000000000000c011"), 7n), listing: { acquisition: "store" as const }, display: { name: "Rainbow Hat" } };

afterEach(cleanup);

it.each<{ root: string; rule: ShouldRevalidateFunction; data: number }>([
  { root: "root's rule", rule: shouldRevalidate, data: 0 },
  { root: "a root that always reloads", rule: () => true, data: 1 },
])("Go to cart preloads the cart page's code and, with $root, $data data requests", async ({ rule, data }) => {
  const cart = createCartStore({ storage: () => null });
  const drawer = createCartDrawer();
  const Stub = createRoutesStub([
    {
      id: "root",
      path: "/",
      loader: () => null,
      shouldRevalidate: rule,
      Component: Outlet,
      children: [
        { path: "shop/items", Component: () => <CartDrawer cart={cart} drawer={drawer} /> },
        { path: "shop/cart", Component: () => null },
      ],
    },
  ]);
  render(<Stub initialEntries={["/shop/items"]} />);
  await act(async () => {
    cart.add(hat);
    drawer.added();
  });
  expect(await screen.findByRole("link", { name: "Go to cart" })).toBeTruthy();
  expect(document.querySelector('link[rel="modulepreload"]')).not.toBeNull();
  expect(document.querySelectorAll('link[rel="prefetch"][as="fetch"]')).toHaveLength(data);
});
