// from: shop@3a322cf9:app/src/components/BuyModal/modal.styles.ts (the shell both checkout dialogs share),
//       app/src/components/BuyModal/BuyModal.tsx (the head, the item row, the no-funds screen, the failure panel),
//       app/src/components/CartCheckoutModal/CartCheckoutModal.tsx (Processing's counted step, NoFunds' line list),
//       app/src/components/CreditPackPicker/CreditPackPicker.tsx, app/src/components/Icons/WarningTriangleIcon.tsx,
//       app/src/intl/en.json (buyModal, cartCheckout)
// The checkout dialogs' shell, shared as upstream's modal.styles is: Buy now's (detail/components/BuyModal.tsx) and the
// cart's (CartCheckoutModal.tsx). It holds the dialog's behaviour (focus moves in and back, Tab stays inside, Escape
// closes, the browser asks before leaving a purchase being sent) and the screens both draw: the head with the Credits
// balance, an item, the packs that finish a purchase the Credits fall short of, the progress and the failure panel.
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type AnchorHTMLAttributes,
  type ButtonHTMLAttributes,
  type CSSProperties,
  type KeyboardEvent,
  type ReactNode,
  type RefObject,
} from "react";

import loaderLogo from "../../credits/assets/loader-logo.svg?url";
import { useCreditPacks } from "../../credits/hooks";
import { isIapMode } from "../../credits/iap";
import { createPackCheckout, MAX_OFFER_PACKS, offerablePacks } from "../../credits/payments";
import CategoryFlap from "../../discovery/components/CategoryFlap";
import Icon from "../../discovery/components/Icon";
import { useCreatorName } from "../../discovery/hooks";
import { rarityCardPalette } from "../../discovery/lib/rarity";
import type { CartLineDisplay } from "../../model/purchase";
import { SHOP_BASE } from "../../routes";
import buyErrorArt from "../assets/buy-error.png?url";
import packCoin from "../assets/pack-coin.webp?url";
import { friendlyError } from "../lib/errors";
import { useShopServices } from "../services";
import styles from "./CheckoutDialog.module.css";


const TEXT = {
  balance: "My Credits Balance:",
  close: "Close",
  insufficient: "Insufficient Funds.",
  needToBuy: "You will need to buy",
  toPurchaseOne: "to purchase this item.",
  toPurchaseMany: "to purchase these items.",
  learnMore: "Learn More",
  cancel: "Cancel",
  buy: "Buy",
  errorHeadline: "We couldn't complete your purchase this time.",
  errorBody: "Don't worry, your Credits are safe. Please try again in a moment.",
  tryAgain: "Try again",
  itemFallback: "Item",
  packsFailed: "Couldn't start the Credits checkout — please try again.",
  inApp: "Get Credits in the Decentraland app, then come back to finish this purchase.",
} as const;

export type DialogHandle = { ref: RefObject<HTMLDivElement | null>; onKeyDown(e: KeyboardEvent): void; focus(): void };

/**
 * A modal dialog's behaviour: focus moves in when it opens and back to what had it (the button that opened it) when it
 * goes, Tab and Shift+Tab stay inside, Escape asks to close (the caller refuses while a purchase runs), and while
 * `busy` the browser asks before the page is left, which would leave a purchase being sent unwatched.
 */
export function useDialog(busy: boolean, onClose: () => void): DialogHandle {
  const ref = useRef<HTMLDivElement>(null);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key === "Escape") closeRef.current();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  useEffect(() => {
    if (!busy) return;
    const hold = (e: BeforeUnloadEvent) => {
      e.preventDefault();
      // Older Chromium and some web views prompt only when this is set.
      e.returnValue = "";
    };
    window.addEventListener("beforeunload", hold);
    return () => window.removeEventListener("beforeunload", hold);
  }, [busy]);
  useEffect(() => {
    const before = typeof document !== "undefined" ? (document.activeElement as HTMLElement | null) : null;
    ref.current?.focus();
    return () => before?.focus?.();
  }, []);
  // One handle for the dialog's life: what it holds reads the ref, so an effect depending on it runs only when it should.
  return useMemo(() => ({ ref, onKeyDown: (e: KeyboardEvent) => trapTab(ref.current, e), focus: () => ref.current?.focus() }), []);
}

/** A modal dialog keeps Tab and Shift+Tab inside it. */
function trapTab(root: HTMLDivElement | null, e: KeyboardEvent) {
  if (e.key !== "Tab" || !root) return;
  const focusable = [...root.querySelectorAll<HTMLElement>('a[href], button:not([disabled]), input:not([disabled]), [tabindex]:not([tabindex="-1"])')];
  if (focusable.length === 0) return;
  const first = focusable[0]!;
  const last = focusable[focusable.length - 1]!;
  if (e.shiftKey && (document.activeElement === first || document.activeElement === root)) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && document.activeElement === last) {
    e.preventDefault();
    first.focus();
  }
}

/** The scrim and the card. A click on the scrim closes, except while a purchase runs. */
export function DialogShell(props: {
  dialog: DialogHandle;
  label: string;
  testId: string;
  busy: boolean;
  onClose(): void;
  /** The processing and loading screens keep the card's full height. */
  tall?: boolean;
  phase?: string;
  method?: boolean;
  children: ReactNode;
}) {
  return (
    <div className={styles.root} data-shop-tokens role="dialog" aria-modal="true" aria-label={props.label} data-testid={props.testId} ref={props.dialog.ref} tabIndex={-1} onKeyDown={props.dialog.onKeyDown}>
      <div className={styles.scrim} onClick={props.busy ? undefined : props.onClose} aria-hidden="true" data-testid={`${props.testId}-scrim`} />
      <div className={styles.card} data-tall={props.tall || undefined} data-phase={props.phase} data-method={props.method || undefined}>
        {props.children}
      </div>
    </div>
  );
}

/** The title, the close button (gone while a purchase runs) and the Credits balance. */
export function DialogHead(props: { title: string; balance: string; busy: boolean; onClose(): void; testId: string; children?: ReactNode }) {
  return (
    <div className={styles.head}>
      <div className={styles.headrow}>
        <h2 className={styles.title}>{props.title}</h2>
        {!props.busy ? (
          <button type="button" className={styles.x} onClick={props.onClose} aria-label={TEXT.close} data-testid={`${props.testId}-close`}>
            <svg viewBox="0 0 18 18" width="18" height="18" aria-hidden="true">
              <path d="M4 4l10 10M14 4L4 14" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
            </svg>
          </button>
        ) : null}
      </div>
      <div className={styles.balance} data-testid={`${props.testId}-balance`}>
        <span className={styles.balanceLabel}>{TEXT.balance}</span>
        <Icon name="credits" className={styles.balanceIco} />
        <span className={styles.balanceValue}>{props.balance}</span>
      </div>
      {props.children}
    </div>
  );
}

const WarningIcon = () => (
  <svg viewBox="0 0 20 20" width="24" height="24" aria-hidden="true">
    <path
      className={styles.warningMark}
      d="M10 0a10 10 0 1 1 0 20 10 10 0 0 1 0-20Zm0 2a8 8 0 1 0 0 16 8 8 0 0 0 0-16Zm0 11a1 1 0 1 1 0 2 1 1 0 0 1 0-2Zm0-8a1 1 0 0 1 1 1v5a1 1 0 1 1-2 0V6a1 1 0 0 1 1-1Z"
    />
  </svg>
);

/** The warm banner with the warning glyph. */
export function DialogWarning({ testId, status, children }: { testId?: string; status?: boolean; children: ReactNode }) {
  return (
    <div className={styles.warning} role={status ? "status" : undefined} data-testid={testId}>
      <WarningIcon />
      <p className={styles.warningText}>{children}</p>
    </div>
  );
}

/** A link-styled button inside a banner (the way back for a wallet). */
export function DialogTextButton({ onClick, children }: { onClick(): void; children: ReactNode }) {
  return (
    <button type="button" className={styles.warningLink} onClick={onClick}>
      {children}
    </button>
  );
}

/** An item: its artwork on its rarity's tile with the category flap, its name (and units), its creator and its price. */
export function DialogAsset({
  display,
  priceCredits,
  quantity = 1,
  testId,
  flapTestId = "dialog-flap",
}: {
  display: CartLineDisplay;
  priceCredits: number;
  quantity?: number;
  testId?: string;
  /** The category flap's test id: each dialog names its own. */
  flapTestId?: string;
}) {
  const creator = useCreatorName(display.creator);
  const tile = { "--buy-tile": rarityCardPalette(display.rarity).background } as CSSProperties;
  return (
    <div className={styles.asset} data-testid={testId}>
      <div className={styles.thumb} style={tile}>
        <CategoryFlap item={{ category: display.category ?? "wearable", wearableCategory: display.wearableCategory, rarity: display.rarity ?? "" }} size="sm" testId={flapTestId} />
        {display.thumbnail ? <img src={display.thumbnail} alt="" /> : null}
      </div>
      <div className={styles.info}>
        <div>
          <div className={styles.name} title={display.name}>
            {display.name || TEXT.itemFallback}
            {quantity > 1 ? <span className={styles.qty}>{`× ${quantity}`}</span> : null}
          </div>
          {display.creator ? <div className={styles.creator}>{creator ? `By ${creator}` : " "}</div> : null}
        </div>
        <div className={styles.price}>
          <Icon name="credits" className={styles.priceIco} />
          <span>{(priceCredits * quantity).toLocaleString("en-US")}</span>
        </div>
      </div>
    </div>
  );
}

/** Several items, in a list that scrolls before the packs and buttons below it leave the screen. */
export function DialogAssetList({ children }: { children: ReactNode }) {
  return (
    <div className={styles.scroll} data-testid="dialog-lines">
      {children}
    </div>
  );
}

/**
 * Not enough Credits and no MANA that can pay: what the buyer is short, the items, and the packs that finish the
 * purchase (all of them when none does), the cheapest of those picked. Buy leaves for the pack checkout through the
 * caller's hand-off (`onLeave`, then `onCheckout` with the order it created); a dialog closed while the checkout is
 * being created never leaves.
 */
export function NoFundsBody(props: {
  /** The whole Credits the buyer must add. */
  shortfall: number;
  /** How many units the purchase holds: "this item" or "these items". */
  units: number;
  /** The items, as `DialogAsset`s (several in a `DialogAssetList`). */
  items: ReactNode;
  /** The cart's banner puts Learn More on a line of its own, as upstream's. */
  learnMoreBelow?: boolean;
  testId: string;
  /** Leaves the hand-off; returns its attempt, which a failure drops. */
  onLeave(): string | null;
  onFailed(attempt: string | null): void;
  onCancel(): void;
  onCheckout(session: { orderId: string; url?: string }): void;
}) {
  const { client } = useShopServices();
  const { packs } = useCreditPacks();
  const offer = useMemo(() => offerablePacks(packs.slice(0, MAX_OFFER_PACKS), props.shortfall), [packs, props.shortfall]);
  const cover = offer.packs.find((p) => p.credits >= props.shortfall) ?? offer.packs[offer.packs.length - 1];
  const [picked, setPicked] = useState<string | null>(null);
  const selectedId = picked ?? cover?.id ?? null;
  const selected = offer.packs.find((p) => p.id === selectedId) ?? null;
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Back from Stripe through the browser's page cache, the page comes back as it was left: Buy pressed.
  useEffect(() => {
    const onShow = (e: PageTransitionEvent) => {
      if (e.persisted) setBusy(false);
    };
    window.addEventListener("pageshow", onShow);
    return () => window.removeEventListener("pageshow", onShow);
  }, []);
  const mounted = useRef(true);
  useEffect(() => {
    // Set here too: StrictMode runs this effect's cleanup and then the effect again.
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  const iap = isIapMode();

  async function buy() {
    if (!selectedId) return;
    setBusy(true);
    setError(null);
    const leaving = props.onLeave();
    try {
      const session = await createPackCheckout(selectedId, client);
      if (!mounted.current) {
        props.onFailed(leaving);
        return;
      }
      props.onCheckout(session);
    } catch (e) {
      props.onFailed(leaving);
      if (!mounted.current) return;
      setError(friendlyError(e, TEXT.packsFailed));
      setBusy(false);
    }
  }

  return (
    <div className={styles.body}>
      <div className={styles.warning} data-testid={`${props.testId}-insufficient`}>
        <WarningIcon />
        <p className={styles.warningText}>
          <b>{TEXT.insufficient}</b> {TEXT.needToBuy} <b>{`${props.shortfall.toLocaleString("en-US")} Credits`}</b> {props.units > 1 ? TEXT.toPurchaseMany : TEXT.toPurchaseOne}
          {iap ? (
            // Credits are sold in the app there: the way forward, since this dialog offers no packs.
            <>
              {props.learnMoreBelow ? <br /> : " "}
              {TEXT.inApp}
            </>
          ) : (
            <>
              {props.learnMoreBelow ? <br /> : " "}
              <a className={styles.warningLink} href={`${SHOP_BASE}/credits`} target="_blank" rel="noopener noreferrer">
                {TEXT.learnMore}
              </a>
            </>
          )}
        </p>
      </div>
      {props.items}
      {iap ? null : (
        <>
          <div className={styles.packs} data-testid="credit-packs">
            {offer.packs.map((p) => (
              <button
                key={p.id}
                type="button"
                className={styles.pack}
                data-testid="credit-pack"
                data-on={p.id === selectedId || undefined}
                aria-pressed={p.id === selectedId}
                aria-label={`${p.credits.toLocaleString("en-US")} Credits for $${p.usd.toFixed(2)}`}
                onClick={() => setPicked(p.id)}
              >
                <img className={styles.packIco} src={packCoin} alt="" />
                <span className={styles.packAmount}>{p.credits.toLocaleString("en-US")}</span>
                <span className={styles.packUsd}>(${p.usd.toFixed(2)})</span>
              </button>
            ))}
          </div>
          <div className={styles.total}>
            <div className={styles.totalCredits}>
              <Icon name="credits" className={styles.totalIco} />
              <span data-testid="topup-total-credits">{(selected?.credits ?? 0).toLocaleString("en-US")}</span>
            </div>
            <span className={styles.totalUsd}>${(selected?.usd ?? 0).toFixed(2)}</span>
          </div>
        </>
      )}
      {error ? (
        <p className={styles.warningText} role="alert">
          {error}
        </p>
      ) : null}
      <div className={styles.ctas}>
        <button type="button" className={styles.btn} data-variant="outline" onClick={props.onCancel}>
          {TEXT.cancel}
        </button>
        {iap ? null : (
          <button type="button" className={styles.btn} data-variant="gradient" onClick={() => void buy()} disabled={busy || !selectedId} data-testid={`${props.testId}-topup`}>
            {TEXT.buy}
          </button>
        )}
      </div>
    </div>
  );
}

/** A screen's body; `processing` centres it in the card's full height, as the loading and progress screens are. */
export function DialogBody({ processing, testId, children }: { processing?: boolean; testId?: string; children: ReactNode }) {
  return (
    <div className={styles.body} data-processing={processing || undefined} data-testid={testId}>
      {children}
    </div>
  );
}

/** The screen's buttons, side by side, or stacked (`stack`). */
export function DialogCtas({ stack, children }: { stack?: boolean; children: ReactNode }) {
  return (
    <div className={styles.ctas} data-stack={stack || undefined}>
      {children}
    </div>
  );
}

type Variant = "gradient" | "outline" | "ruby";

/** A dialog button: the gradient Buy, the outline Cancel, the ruby retry. `full` takes the row's width. */
export function DialogButton({ variant, full, ...rest }: { variant: Variant; full?: boolean } & ButtonHTMLAttributes<HTMLButtonElement>) {
  return <button type="button" {...rest} className={styles.btn} data-variant={variant} data-full={full || undefined} />;
}

/** A link drawn as a dialog button (Try in world, Go to backpack). */
export function DialogLinkButton({ variant, ...rest }: { variant: Variant } & AnchorHTMLAttributes<HTMLAnchorElement>) {
  return <a {...rest} className={styles.btn} data-variant={variant} />;
}

/** The first price check: a spinner in the card's full height. */
export function LoadingBody() {
  return (
    <div className={styles.body} data-processing>
      <span className={styles.spinner} aria-hidden="true" />
    </div>
  );
}

/** A purchase sent and not confirmed yet: the loader, what to expect, and the screen's buttons. */
export function PendingBody({ text, testId, children }: { text: string; testId?: string; children: ReactNode }) {
  return (
    <div className={styles.body} data-testid={testId}>
      <div className={styles.success}>
        <img className={styles.logo} src={loaderLogo} alt="" width={61} height={61} />
        <p className={styles.processing}>{text}</p>
      </div>
      {children}
    </div>
  );
}

/** A purchase on its way: the loader, what is happening, and a sliding bar that claims no progress it has not made. */
export function ProcessingBody({ text }: { text: string }) {
  return (
    <div className={styles.body} data-processing>
      <img className={styles.logo} src={loaderLogo} alt="" width={61} height={61} />
      <div className={styles.processing}>{text}</div>
      <div className={styles.progress} aria-hidden="true">
        <span className={styles.progressFill} />
      </div>
    </div>
  );
}

/** The failure panel: the reason (or that the Credits are safe), and a retry when there is one to make. */
export function ErrorBody({ message, onClose, onRetry }: { message: string | null; onClose(): void; onRetry?: () => void }) {
  return (
    <div className={styles.body}>
      <div className={styles.error} data-testid="buy-error">
        <img className={styles.errorArt} src={buyErrorArt} alt="" width={64} height={80} />
        <p className={styles.errorText}>
          <b>{TEXT.errorHeadline}</b> {message ?? TEXT.errorBody}
        </p>
      </div>
      <div className={styles.ctas}>
        <button type="button" className={styles.btn} data-variant="outline" data-full={onRetry ? undefined : true} onClick={onClose}>
          {TEXT.cancel}
        </button>
        {onRetry ? (
          <button type="button" className={styles.btn} data-variant="ruby" onClick={onRetry}>
            {TEXT.tryAgain}
          </button>
        ) : null}
      </div>
    </div>
  );
}
