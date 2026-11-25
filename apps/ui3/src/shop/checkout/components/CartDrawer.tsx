// from: shop@3a322cf9:app/src/components/CartPopover/CartPopover.tsx (CartRow, CartPopover)
// The cart drawer: a right-side slide-in panel that opens (a) when an item is added to the cart, with a
// success banner, and (b) when the cart icon in the subnav is clicked (no banner). It never starts a
// charge: its primary CTA goes to the cart, where the buyer sees what they are buying first.
import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type RefObject } from "react";
import { createPortal } from "react-dom";
import { Link, useLocation } from "react-router";

import { shopCartDrawer, useCartDrawer, type CartDrawerStore } from "../../cart/drawer";
import { shopCart, useCart, type CartStore } from "../../cart/store";
import { DetailCreator } from "../../detail/components/DetailBadges";
import CategoryFlap from "../../discovery/components/CategoryFlap";
import Icon from "../../discovery/components/Icon";
import { CreditsAmount } from "../../discovery/components/PriceTag";
import { categoryIcon } from "../../discovery/lib/itemIcons";
import { rarityCardPalette } from "@shop/discovery/lib/rarity";
import { creditsCeil } from "@shop/model/money";
import { offerQuote, type CartLine } from "@shop/model/purchase";
import { SHOP_BASE } from "../../routes";
import cartEmptyIllustration from "../assets/cart-empty.svg?url";
import styles from "./CartDrawer.module.css";

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
  const stock = lineStock(line);
  const atStockCap = typeof stock === "number" && quantity >= stock;
  const unit = drawerUnitCredits(line);
  const palette = rarityCardPalette(line.display.rarity);
  const flapItem = line.display.category ? { category: line.display.category, wearableCategory: line.display.wearableCategory, rarity: line.display.rarity ?? "" } : null;
  return (
    <li className={styles.card} data-testid="cart-drawer-line">
      <div className={styles.thumb} data-thumb style={{ "--tile-bg": palette.background, "--tile-stroke": palette.stroke } as CSSProperties}>
        {line.display.thumbnail ? <img src={line.display.thumbnail} alt={line.display.name} /> : null}
        {flapItem && categoryIcon(flapItem) ? <CategoryFlap item={flapItem} size="sm" testId="cart-flap" /> : null}
      </div>
      <div className={styles.info}>
        <div data-desc>
          <div className={styles.name} title={line.display.name}>
            {line.display.name}
          </div>
          {line.display.creator ? <DetailCreator address={line.display.creator} size="sm" className={styles.by} /> : null}
        </div>
        <div className={styles.bottom}>
          {isMint ? (
            <div className={styles.stepper}>
              <button type="button" className={styles.step} onClick={() => cart.decrement(line.id)} disabled={quantity <= 1} aria-label={`Decrease quantity of ${line.display.name}`}>
                <Minus />
              </button>
              <span className={styles.qty}>{quantity}</span>
              <button type="button" className={styles.step} onClick={() => cart.increment(line.id, stock)} disabled={atStockCap} aria-label={`Increase quantity of ${line.display.name}`}>
                <Plus />
              </button>
            </div>
          ) : null}
          {unit !== null ? <CreditsAmount credits={unit * quantity} className={styles.price} coinClassName={styles.coin} /> : null}
        </div>
      </div>
      <button
        type="button"
        className={styles.del}
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

// As library/ui's Modal. A browser without modal dialogs (none in the build target; jsdom in tests) still shows and
// hides the drawer, but without the inert page, the focus move or Escape.
function show(dialog: HTMLDialogElement) {
  if (dialog.open) return;
  if (typeof dialog.showModal === "function") dialog.showModal();
  else dialog.setAttribute("open", "");
}

function hide(dialog: HTMLDialogElement) {
  if (!dialog.open) return;
  if (typeof dialog.close === "function") dialog.close();
  else dialog.removeAttribute("open");
}

// Go to cart preloads the cart page's code once the panel has slid in (a cancelled slide counts), so the slide keeps
// the main thread and the network. Root's data does not reload between Shop pages either (shouldRevalidate in
// apps/sites/src/routes/root.tsx), so the click waits for no server request. The in-world HUD's router has no
// preload, so there it stays a plain link. Go to cart only navigates: the drawer closes when the page changes, so
// the click does not wait for the dialog's close either. On the cart page the link changes no page, so there the
// click closes the drawer.
function DrawerCtas({ panel, onClose }: { panel: RefObject<HTMLElement | null>; onClose: () => void }) {
  const [slid, setSlid] = useState(false);
  const onCartPage = useLocation().pathname === `${SHOP_BASE}/cart`;
  useEffect(() => {
    const settle = () => setSlid(true);
    void Promise.all((panel.current?.getAnimations?.() ?? []).map((a) => a.finished)).then(settle, settle);
  }, [panel]);
  return (
    <div className={styles.ctas}>
      <button type="button" className={styles.cta} data-variant="glass" onClick={onClose}>
        Continue shopping
      </button>
      <Link to={`${SHOP_BASE}/cart`} prefetch={slid ? "render" : "none"} className={styles.cta} data-variant="primary" onClick={onCartPage ? onClose : undefined}>
        Go to cart
      </Link>
    </div>
  );
}

export default function CartDrawer({ cart = shopCart, drawer = shopCartDrawer }: { cart?: CartStore; drawer?: CartDrawerStore }) {
  const lines = useCart((s) => s.lines, cart);
  const { open, justAdded } = useCartDrawer(drawer);
  const dialogRef = useRef<HTMLDialogElement>(null);
  const panelRef = useRef<HTMLElement>(null);
  const { pathname } = useLocation();

  // The dialog leaves the document only after its own close() has given focus back.
  const [mounted, setMounted] = useState(false);
  if (open && !mounted) setMounted(true);

  // A modal <dialog>: the browser moves focus into it, keeps Tab inside, makes the page inert, closes it on Escape,
  // and on close gives focus back where it was without a scroll of the page. No auto-dismiss: it stays until
  // dismissed. A layout effect, so the panel's slide has started before DrawerCtas looks for it.
  useLayoutEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    if (open) {
      show(dialog);
    } else {
      hide(dialog);
      setMounted(false);
    }
  }, [open, mounted]);

  // Navigating away (a link in it, the browser's back) closes it: it belongs to the page it opened over. Leaving the
  // Shop unmounts it, so that closes it too, and it does not open again by itself on the next Shop page.
  const openedOn = useRef(pathname);
  useEffect(() => {
    if (!open) {
      openedOn.current = pathname;
      return;
    }
    if (pathname !== openedOn.current) drawer.close();
  }, [open, pathname, drawer]);
  useEffect(() => () => drawer.close(), [drawer]);

  if (!mounted || typeof document === "undefined") return null;

  // The count is the units (Σ quantity), not the distinct lines. A total with a line it cannot price would
  // read as complete while it is not, so it is shown only when every line has a price.
  const count = lines.reduce((n, l) => n + l.quantity, 0);
  const units = lines.map((l) => drawerUnitCredits(l));
  const total = units.every((u) => u !== null) ? lines.reduce((sum, l, i) => sum + units[i]! * l.quantity, 0) : null;
  const isEmpty = lines.length === 0;
  const close = () => drawer.close();

  // Portalled to <body>, outside the page's stacking contexts. The close event follows a close the browser makes
  // itself (Escape, after its cancel event); a dialog already shown again by then stays open.
  return createPortal(
    <dialog
      ref={dialogRef}
      className={styles.root}
      aria-label="Shopping cart"
      data-testid="cart-drawer"
      onClose={() => {
        if (!dialogRef.current?.open) close();
      }}
    >
      <div className={styles.scrim} onClick={close} />
      <aside className={styles.panel} ref={panelRef}>
        <header className={styles.head}>
          <h2 className={styles.title}>{`Shopping Cart (${count})`}</h2>
          <button type="button" className={styles.close} onClick={close} aria-label="Close">
            <Icon name="close" size={18} />
          </button>
        </header>

        <div className={styles.body}>
          {isEmpty ? (
            <div className={styles.empty} data-testid="cart-drawer-empty">
              <img className={styles.emptyArt} src={cartEmptyIllustration} alt="" aria-hidden />
              <p className={styles.emptyTitle}>Nothing here... yet!</p>
              <p className={styles.emptyBody}>Explore the Shop and discover your next favorite item.</p>
              <Link to={`${SHOP_BASE}/items`} className={styles.cta} data-variant="glass" onClick={close}>
                Start shopping
              </Link>
            </div>
          ) : null}

          {justAdded > 0 ? (
            <div className={styles.banner} data-testid="cart-drawer-banner">
              <span className={styles.bannerCheck}>
                <CheckCircle />
              </span>
              <p>
                <strong>{justAdded === 1 ? "1 Item" : `${justAdded} Items`}</strong> successfully added to cart!
              </p>
            </div>
          ) : null}

          <ul className={styles.list}>
            {lines.map((line) => (
              <DrawerLine key={line.id} line={line} cart={cart} drawer={drawer} />
            ))}
          </ul>
        </div>

        {/* No total and nothing to check out on an empty cart: the empty state carries its own CTA. */}
        {isEmpty ? null : (
          <footer className={styles.foot}>
            <div className={styles.total}>
              <span className={styles.totalLabel}>Total: {count === 1 ? "1 item" : `${count} items`}</span>
              {total !== null ? <CreditsAmount credits={total} className={styles.totalVal} coinClassName={styles.coin} testId="cart-drawer-total" /> : null}
            </div>
            {/* Dismiss on the left, advance on the right, and advancing stops at the cart. */}
            <DrawerCtas panel={panelRef} onClose={close} />
          </footer>
        )}
      </aside>
    </dialog>,
    document.body,
  );
}
