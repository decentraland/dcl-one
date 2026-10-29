// The detail page's hand-off to a purchase. Upstream opened its BuyModal from the page; the purchase
// flows (approvals, signatures, relay, progress) are ui3/src/shop/purchase's, so this is a seam: the
// page says WHAT to buy (the server offer the buyer saw and the method they chose) and the installed
// launcher runs it. Without one, the purchase goes through the cart, whose checkout partitions,
// revalidates and settles it (cart/), which is the same path a multi-item basket takes.

import { createContext, useContext, type ReactNode } from "react";

import { cartLineId, shopCart, type CartAddInput, type CartStore } from "../cart/store";
import type { PurchaseOffer } from "../model/purchase";
import type { PaymentMethod } from "../model/settlement";
import { SHOP_BASE } from "../routes";

export type PurchaseRequest = {
  line: CartAddInput;
  /** The buyer's choice; the presentation default when null. */
  method: PaymentMethod | null;
  offer: PurchaseOffer | null;
};

export type PurchaseLauncher = (request: PurchaseRequest, navigate: (to: string) => void) => void;

/** Puts the line (and the method chosen) in the account's cart and opens the cart's checkout. */
export function cartLauncher(cart: CartStore = shopCart): PurchaseLauncher {
  return ({ line, method, offer }, navigate) => {
    const id = cartLineId(line.asset, line.listing);
    if (!cart.getSnapshot().lines.some((l) => l.id === id)) cart.add({ ...line, seen: offer ?? line.seen ?? null });
    else if (offer) cart.markSeen(id, offer);
    cart.setMethod(id, method);
    navigate(`${SHOP_BASE}/cart`);
  };
}

const LauncherContext = createContext<PurchaseLauncher>(cartLauncher());

export function ShopPurchaseProvider({ launcher, children }: { launcher: PurchaseLauncher; children: ReactNode }) {
  return <LauncherContext.Provider value={launcher}>{children}</LauncherContext.Provider>;
}

export function usePurchaseLauncher(): PurchaseLauncher {
  return useContext(LauncherContext);
}
