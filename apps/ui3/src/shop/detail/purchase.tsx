// The detail page's hand-off to a purchase. Upstream opened its BuyModal from the page; the purchase
// flows (approvals, signatures, relay, progress) are ui3/src/shop/purchase's, so this is a seam: the
// page says WHAT to buy (the server offer the buyer saw and the method they chose) and the installed
// launcher runs it. Without one, the purchase goes through the cart, whose checkout partitions,
// revalidates and settles it (cart/), which is the same path a multi-item basket takes.

import { createContext, useContext, type ReactNode } from "react";

import { shopCartDrawer, type CartDrawerStore } from "../cart/drawer";
import { cartLineId, shopCart, type CartAddInput, type CartStore } from "../cart/store";
import type { PurchaseOffer } from "@shop/model/purchase";
import type { PaymentMethod } from "@shop/model/settlement";
import { SHOP_BASE } from "../routes";

export type PurchaseRequest = {
  line: CartAddInput;
  /** The buyer's choice; the presentation default when null. */
  method: PaymentMethod | null;
  offer: PurchaseOffer | null;
  /** "add" puts the line in the cart and stays on the page; "buy" (the default) goes on to the checkout. */
  intent?: "add" | "buy";
};

export type PurchaseLauncher = (request: PurchaseRequest, navigate: (to: string) => void) => void;

/**
 * Puts the line (and the method chosen) in the account's cart. Buying opens the cart's checkout; adding
 * opens the cart drawer over the page, as upstream's add-to-cart does (shop@3a322cf9 store/cart.ts `add`).
 */
export function cartLauncher(cart: CartStore = shopCart, drawer: CartDrawerStore = shopCartDrawer): PurchaseLauncher {
  return ({ line, method, offer, intent = "buy" }, navigate) => {
    const id = cartLineId(line.asset, line.listing);
    const before = cart.getSnapshot().lines.find((l) => l.id === id)?.quantity ?? 0;
    if (before === 0) cart.add({ ...line, seen: offer ?? line.seen ?? null });
    else if (intent === "add") {
      cart.add(line);
      if (offer) cart.markSeen(id, offer);
    } else if (offer) cart.markSeen(id, offer);
    cart.setMethod(id, method);
    if (intent === "buy") navigate(`${SHOP_BASE}/cart`);
    // The banner counts what the cart took: a token already in it, or a mint at its stock, adds nothing.
    else if ((cart.getSnapshot().lines.find((l) => l.id === id)?.quantity ?? 0) > before) drawer.added();
    else drawer.open();
  };
}

const LauncherContext = createContext<PurchaseLauncher>(cartLauncher());
const InPlaceContext = createContext(true);

/**
 * A host's own purchase flow. Buy now then goes through `launcher` rather than over the page, unless the host says the
 * page's dialog is still the way it buys (`buysInPlace`).
 */
export function ShopPurchaseProvider({ launcher, buysInPlace = false, children }: { launcher: PurchaseLauncher; buysInPlace?: boolean; children: ReactNode }) {
  return (
    <LauncherContext.Provider value={launcher}>
      <InPlaceContext.Provider value={buysInPlace}>{children}</InPlaceContext.Provider>
    </LauncherContext.Provider>
  );
}

export function usePurchaseLauncher(): PurchaseLauncher {
  return useContext(LauncherContext);
}

/**
 * Whether Buy now buys over the page (upstream's BuyModal): the web Shop, unless a host installed a purchase flow of its
 * own (ShopPurchaseProvider).
 */
export function useBuysInPlace(): boolean {
  return useContext(InPlaceContext);
}
