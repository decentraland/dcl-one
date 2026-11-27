// from: shop@3a322cf9:app/src/components/CartPopover/CartPopover.tsx (CartRow, CartPopover)
// The cart drawer: a right-side slide-in panel that opens (a) when an item is added to the cart, with a
// success banner, and (b) when the cart icon in the subnav is clicked (no banner). It never starts a
// charge: its primary CTA goes to the cart, where the buyer sees what they are buying first.
import { useEffect, useRef } from "react";
import { ShopCartDrawer, type ShopCartDrawerLine } from "@dcl/one-ui";
import { Link, useLocation } from "react-router";

import { shopCartDrawer, useCartDrawer, type CartDrawerStore } from "../../cart/drawer";
import { shopCart, useCart, type CartStore } from "../../cart/store";
import { DetailCreator } from "../../detail/components/DetailBadges";
import CategoryFlap from "../../discovery/components/CategoryFlap";
import { CreditsAmount } from "../../discovery/components/PriceTag";
import { categoryIcon } from "../../discovery/lib/itemIcons";
import Icon from "../../discovery/components/Icon";
import { creditsCeil } from "@shop/model/money";
import { offerQuote, type CartLine } from "@shop/model/purchase";
import { SHOP_BASE } from "../../routes";
import cartEmptyIllustration from "../assets/cart-empty.svg?url";

/** The line's price in credits per unit: what the catalog showed, else the offer the buyer saw. */
export function drawerUnitCredits(line: CartLine): number | null {
  if (typeof line.display.priceCredits === "number") return line.display.priceCredits;
  const quote = line.seen ? offerQuote(line.seen, "credits") : null;
  return quote?.method === "credits" ? creditsCeil(quote.quoted) : null;
}

/** A mint's stock: the offer the buyer saw, else what the card or the page showed when it was added. */
function lineStock(line: CartLine): number | null {
  return line.seen?.available ?? line.available ?? null;
}

export default function CartDrawer({ cart = shopCart, drawer = shopCartDrawer }: { cart?: CartStore; drawer?: CartDrawerStore }) {
  const lines = useCart((s) => s.lines, cart);
  const { open, justAdded } = useCartDrawer(drawer);
  const { pathname } = useLocation();
  // Navigating away (a link in it, the browser's back) closes it: it belongs to the page it opened over.
  const openedOn = useRef(pathname);
  useEffect(() => {
    if (!open) {
      openedOn.current = pathname;
      return;
    }
    if (pathname !== openedOn.current) drawer.close();
  }, [open, pathname, drawer]);
  useEffect(() => () => drawer.close(), [drawer]);

  const getContents = () => {
    const viewLines: ShopCartDrawerLine[] = lines.map((line) => {
      const stock = lineStock(line);
      const unit = drawerUnitCredits(line);
      const { name, thumbnail, creator, category, wearableCategory, rarity } = line.display;
      const flapItem = category ? { category, wearableCategory, rarity: rarity ?? "" } : null;
      return {
        id: line.id,
        name,
        thumbnail,
        rarity,
        quantity: line.quantity,
        flap: flapItem && categoryIcon(flapItem) ? <CategoryFlap item={flapItem} size="sm" testId="cart-flap" /> : null,
        creator: creator ? (className) => <DetailCreator address={creator} size="sm" className={className} /> : undefined,
        price: unit === null ? null : unit * line.quantity,
        stepper:
          line.asset.token.kind === "item"
            ? {
                atStockCap: typeof stock === "number" && line.quantity >= stock,
                increase: () => cart.increment(line.id, stock),
                decrease: () => cart.decrement(line.id),
              }
            : undefined,
        remove: () => {
          cart.remove(line.id);
          drawer.clearBanner();
        },
      };
    });
    const count = viewLines.reduce((sum, line) => sum + line.quantity, 0);
    const total = viewLines.reduce<number | null>((sum, line) => (sum === null || line.price === null ? null : sum + line.price), 0);
    return { count, lines: viewLines, total };
  };
  return (
    <ShopCartDrawer
      open={open}
      justAdded={justAdded}
      getContents={getContents}
      credits={(amount, className, coinClassName, testId) => (
        <CreditsAmount credits={amount} className={className} coinClassName={coinClassName} testId={testId} />
      )}
      cartHref={`${SHOP_BASE}/cart`}
      itemsHref={`${SHOP_BASE}/items`}
      onCartPage={pathname === `${SHOP_BASE}/cart`}
      Link={Link}
      icons={{ close: <Icon name="close" size={18} />, remove: <Icon name="trash" size={24} /> }}
      emptyArt={cartEmptyIllustration}
      close={() => drawer.close()}
    />
  );
}
