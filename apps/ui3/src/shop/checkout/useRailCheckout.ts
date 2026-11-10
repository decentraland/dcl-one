// from: shop@3a322cf9:app/src/pages/Cart.tsx (onPay: a rail's button pays the cart on that rail). dcl-one keeps a
// method per line, so a rail first moves the lines to it; the cart then re-prices on that method before anything
// is charged. A charge goes out on its own only for an amount the buyer saw on the button they pressed: otherwise
// the re-priced cart stays on screen, with its new total, for a second press.
import { useEffect, useState } from "react";

import { revalidationIdentity } from "../cart/revalidate";
import type { CartPartition } from "@shop/cart/partition";
import type { CartLine } from "@shop/model/purchase";
import type { PaymentMethod } from "@shop/model/settlement";
import type { CartRail } from "./lib/cart-rails";
import type { CheckoutView } from "./useCheckout";

/** Shown when a rail re-priced the cart onto a total the buyer has not seen yet. */
export const REPRICED_NOTICE = "Your cart is now priced for this payment. Check the total, then pay.";

type Pending = {
  /** The price check that must finish first: the lines with their new methods. */
  target: string;
  /** The MANA amount the pressed button showed, on its chain; null when the button showed no amount. */
  shown: { manaWei: bigint; chainId: number } | null;
};

/** The cart's MANA on one chain, as the partition prices it. */
export function partitionMana(partition: CartPartition | null, chainId: number): bigint {
  let sum = 0n;
  for (const g of partition?.groups ?? []) if (g.total.method === "mana" && g.key.chainId === chainId) sum += g.total.amount;
  return sum;
}

export function useRailCheckout(co: CheckoutView, buyable: readonly CartLine[], openTopUp: () => void) {
  const [pending, setPending] = useState<Pending | null>(null);
  // The notice speaks for one price check: once the cart changes or re-prices, it no longer describes the total.
  const [notice, setNotice] = useState<{ text: string; pricedFor: string | null } | null>(null);

  // What a press does once the cart is priced as it will be paid. Opening the top-up charges nothing.
  const proceed = (charge: boolean) => {
    if (co.canConfirm) {
      if (charge) void co.confirm();
      else setNotice({ text: REPRICED_NOTICE, pricedFor: co.pricedFor });
    } else if (co.topUp && co.review.kind === "current") openTopUp();
  };

  // The move restarts the price check; this waits for the check that priced exactly the moved lines.
  useEffect(() => {
    if (!pending || co.running) return;
    // The cart changed under the wait (a line edited or removed, another account's cart): that price check will
    // never come, and nothing the buyer pressed for is on screen any more.
    if (co.review.kind === "outage" || revalidationIdentity(co.lines) !== pending.target) {
      setPending(null);
      return;
    }
    if (co.review.kind === "checking" || co.pricedFor !== pending.target) return;
    setPending(null);
    const shown = pending.shown;
    proceed(!!shown && partitionMana(co.partition, shown.chainId) === shown.manaWei);
  });

  const move = (target: (l: CartLine) => PaymentMethod | null, shown: Pending["shown"]) => {
    setNotice(null);
    // A press is the buyer's own checkout: a resume armed by an earlier sign-in must not run one beside it.
    co.holdAutoConfirm();
    const moving = buyable.filter((l) => l.method !== target(l));
    if (moving.length === 0) {
      // Nothing changes: the summary on screen is what gets charged.
      proceed(true);
      return;
    }
    const ids = new Set(moving.map((l) => l.id));
    for (const l of moving) co.setMethod(l.id, target(l));
    setPending({ target: revalidationIdentity(co.lines.map((l) => (ids.has(l.id) ? { ...l, method: target(l) } : l))), shown });
  };

  return {
    paying: pending !== null,
    notice: notice && notice.pricedFor === co.pricedFor && notice.pricedFor === revalidationIdentity(co.lines) ? notice.text : null,
    /** A rail's button: every line on that method (a line already on it by default stays as it is). */
    pay(rail: CartRail) {
      const method = rail.method;
      move((l) => (l.method === null && co.views[l.id]?.primary === method ? null : method), rail.method === "mana" ? { manaWei: rail.manaWei, chainId: rail.chainId } : null);
    },
    /** No rail covers the cart: each line back on its default way to pay, then the top-up or the checkout. */
    fallback() {
      move(() => null, null);
    },
  };
}
