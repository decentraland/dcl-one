// from: shop@3a322cf9:app/src/components/CartPopover/CartPopover.tsx (CartRow, CartPopover)
// The cart drawer: a right-side slide-in panel that opens (a) when an item is added to the cart, with a
// success banner, and (b) when the cart icon in the subnav is clicked (no banner). It never starts a
// charge: its primary CTA goes to the cart, where the buyer sees what they are buying first.
import { useEffect, type CSSProperties } from "react";
import { createPortal } from "react-dom";
import { Link } from "react-router";

import Icon from "../../discovery/components/Icon";
import CategoryFlap from "../../discovery/components/CategoryFlap";
import { CreditsAmount } from "../../discovery/components/PriceTag";
import { categoryIcon } from "../../discovery/lib/itemIcons";
import { rarityCardPalette } from "../../discovery/lib/rarity";
import { DetailCreator } from "../../detail/components/DetailBadges";
import { shopCart, useCart, type CartStore } from "../../cart/store";
import { shopCartDrawer, useCartDrawer, type CartDrawerStore } from "../../cart/drawer";
import { creditsCeil } from "../../model/money";
import { offerQuote, type CartLine } from "../../model/purchase";
import { SHOP_BASE } from "../../routes";
import cartEmptyIllustration from "../assets/cart-empty.svg?url";
import "./cartdrawer.css";

/** The line's price in credits per unit: what the catalog showed, else the offer the buyer saw. */
export function drawerUnitCredits(line: CartLine): number | null {
  if (typeof line.display.priceCredits === "number") return line.display.priceCredits;
  const quote = line.seen ? offerQuote(line.seen, "credits") : null;
  return quote?.method === "credits" ? creditsCeil(quote.quoted) : null;
}

const Minus = () => (
  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden>
    <path d="M3.5 8h9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
);

const Plus = () => (
  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden>
    <path d="M8 3.5v9M3.5 8h9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
);

const CheckCircle = () => (
  <svg viewBox="0 0 20 20" fill="none" aria-hidden focusable="false">
    <circle cx="10" cy="10" r="10" fill="#1ea672" />
    <path d="M5.8 10.3l2.7 2.7 5.7-6" stroke="#fff" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

// A single cart line: thumbnail, name, creator, quantity stepper, price, delete. A primary mint holds
// several copies (minus floored at 1, plus up to the remaining stock) and shows the line subtotal; an
// existing token is unique, so it has no stepper.
function DrawerLine({ line, cart, drawer }: { line: CartLine; cart: CartStore; drawer: CartDrawerStore }) {
  const isMint = line.asset.token.kind === "item";
  const quantity = line.quantity;
  const available = line.seen?.available ?? null;
  const atStockCap = typeof available === "number" && quantity >= available;
  const unit = drawerUnitCredits(line);
  const palette = rarityCardPalette(line.display.rarity);
  const flapItem = line.display.category ? { category: line.display.category, wearableCategory: line.display.wearableCategory, rarity: line.display.rarity ?? "" } : null;
  return (
    <li className="shop-drawer__card" data-testid="cart-drawer-line">
      <div className="shop-drawer__thumb" data-thumb style={{ "--tile-bg": palette.background, "--tile-stroke": palette.stroke } as CSSProperties}>
        {line.display.thumbnail ? <img src={line.display.thumbnail} alt={line.display.name} /> : null}
        {flapItem && categoryIcon(flapItem) ? <CategoryFlap item={flapItem} size="sm" testId="cart-flap" /> : null}
      </div>
      <div className="shop-drawer__info">
        <div data-desc>
          <div className="shop-drawer__name" title={line.display.name}>
            {line.display.name}
          </div>
          {line.display.creator ? <DetailCreator address={line.display.creator} size="sm" className="shop-drawer__by" /> : null}
        </div>
        <div className="shop-drawer__bottom">
          {isMint ? (
            <div className="shop-drawer__stepper">
              <button type="button" className="shop-drawer__step" onClick={() => cart.decrement(line.id)} disabled={quantity <= 1} aria-label={`Decrease quantity of ${line.display.name}`}>
                <Minus />
              </button>
              <span className="shop-drawer__qty">{quantity}</span>
              <button type="button" className="shop-drawer__step" onClick={() => cart.increment(line.id, available)} disabled={atStockCap} aria-label="Increase quantity">
                <Plus />
              </button>
            </div>
          ) : null}
          {unit !== null ? <CreditsAmount credits={unit * quantity} className="shop-drawer__price" coinClassName="shop-drawer__coin" /> : null}
        </div>
      </div>
      <button
        type="button"
        className="shop-drawer__del"
        onClick={() => {
          cart.remove(line.id);
          drawer.clearBanner();
        }}
        aria-label={`Remove ${line.display.name}`}
        title="Remove"
      >
        <Icon name="trash" size={24} />
      </button>
    </li>
  );
}

export default function CartDrawer({ cart = shopCart, drawer = shopCartDrawer }: { cart?: CartStore; drawer?: CartDrawerStore }) {
  const lines = useCart((s) => s.lines, cart);
  const { open, justAdded } = useCartDrawer(drawer);

  // Escape closes the drawer (outside-click is the scrim's). No auto-dismiss: it stays until dismissed.
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") drawer.close();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, drawer]);

  if (!open || typeof document === "undefined") return null;

  // The count is the units (Σ quantity), not the distinct lines.
  const count = lines.reduce((n, l) => n + l.quantity, 0);
  const total = lines.reduce((sum, l) => sum + (drawerUnitCredits(l) ?? 0) * l.quantity, 0);
  const isEmpty = lines.length === 0;
  const close = () => drawer.close();

  // Portalled to <body> so it overlays the whole viewport, the fixed site nav included.
  return createPortal(
    <div className="shop-drawer" role="dialog" aria-modal="true" aria-label="Shopping cart" data-testid="cart-drawer">
      <div className="shop-drawer__scrim" onClick={close} />
      <aside className="shop-drawer__panel">
        <header className="shop-drawer__head">
          <h2 className="shop-drawer__title">Shopping Cart ({count})</h2>
          <button type="button" className="shop-drawer__close" onClick={close} aria-label="Close">
            <Icon name="close" size={18} />
          </button>
        </header>

        <div className="shop-drawer__body">
          {isEmpty ? (
            <div className="shop-drawer__empty" data-testid="cart-drawer-empty">
              <img className="shop-drawer__empty-art" src={cartEmptyIllustration} alt="" aria-hidden />
              <p className="shop-drawer__empty-title">Nothing here... yet!</p>
              <p className="shop-drawer__empty-body">Explore the Shop and discover your next favorite item.</p>
              <Link to={`${SHOP_BASE}/items`} className="shop-drawer__cta" data-variant="glass" onClick={close}>
                Start shopping
              </Link>
            </div>
          ) : null}

          {justAdded > 0 ? (
            <div className="shop-drawer__banner" data-testid="cart-drawer-banner">
              <span className="shop-drawer__banner-check">
                <CheckCircle />
              </span>
              <p>
                <strong>{justAdded === 1 ? "1 Item" : `${justAdded} Items`}</strong> successfully added to cart!
              </p>
            </div>
          ) : null}

          <ul className="shop-drawer__list">
            {lines.map((line) => (
              <DrawerLine key={line.id} line={line} cart={cart} drawer={drawer} />
            ))}
          </ul>
        </div>

        {/* No total and nothing to check out on an empty cart: the empty state carries its own CTA. */}
        {isEmpty ? null : (
          <footer className="shop-drawer__foot">
            <div className="shop-drawer__total">
              <span className="shop-drawer__total-label">Total: {count === 1 ? "1 item" : `${count} items`}</span>
              <CreditsAmount credits={total} className="shop-drawer__total-val" coinClassName="shop-drawer__coin" />
            </div>
            {/* Dismiss on the left, advance on the right, and advancing stops at the cart. */}
            <div className="shop-drawer__ctas">
              <button type="button" className="shop-drawer__cta" data-variant="glass" onClick={close}>
                Continue shopping
              </button>
              <Link to={`${SHOP_BASE}/cart`} className="shop-drawer__cta" data-variant="primary" onClick={close}>
                Go to cart
              </Link>
            </div>
          </footer>
        )}
      </aside>
    </div>,
    document.body,
  );
}
