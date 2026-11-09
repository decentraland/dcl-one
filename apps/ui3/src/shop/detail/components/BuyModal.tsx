// from: shop@3a322cf9:app/src/components/BuyModal/BuyModal.tsx (the phases, the copy, AssetRow, buyCreditsAndItem's hand-off,
//       the resume), app/src/components/CreditPackPicker/CreditPackPicker.tsx, app/src/components/Icons/{CloseIcon,
//       WarningTriangleIcon,SuccessCheckIcon}.tsx, app/src/intl/en.json (buyModal, cartCheckout)
// Buy now on the detail page, signed in: upstream's dialog over the page instead of a payment choice on it. It is the
// cart's own checkout (useCheckout: review, rails, approvals, the executor) run over a cart of its own holding this one
// line, so the purchase is priced, approved and settled exactly as the cart would, without touching the buyer's cart.
// That cart lives in this tab's session storage, one per account and item, and outlives the dialog: a purchase still
// being settled when the dialog closes or the page reloads is still there, watched and unrepeatable, when Buy now opens
// again. What the dialog adds is upstream's screens:
//   - loading while the line is priced and the balance read;
//   - "Buy Credits and Item" when the Credits do not cover it and no MANA can: the shortfall, the item, the packs that
//     finish the purchase and the Stripe hand-off. Back from it with the record of what the buyer confirmed
//     (credits/returns.ts), the dialog finishes on its own, for the same account, at that price or less;
//   - "Buy Item" when the Credits cover it; a buyer holding MANA chooses how to pay first;
//   - the approval a self-custody wallet grants, the progress, a purchase still being confirmed, "Purchase complete!"
//     and the error.
import { useEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { createPortal } from "react-dom";
import { useNavigate } from "react-router";

import { cartLineId, createCartStore, type CartAddInput, type CartStorage, type CartStore } from "../../cart/store";
import AuthorizeStep from "../../checkout/components/AuthorizeStep";
import PaymentChoice from "../../checkout/components/PaymentChoice";
import { friendlyError } from "../../checkout/lib/errors";
import { EnsureQueryClient, ShopServicesProvider, useShopServices } from "../../checkout/services";
import { useCheckout, type PurchasedLine } from "../../checkout/useCheckout";
import loaderLogo from "../../credits/assets/loader-logo.svg?url";
import { useCreditPacks } from "../../credits/hooks";
import { isIapMode } from "../../credits/iap";
import { createPackCheckout, MAX_OFFER_PACKS, offerablePacks } from "../../credits/payments";
import { bindBuyNowTopUp, dropBuyNowTopUp, stashBuyNowTopUp, type BuyNowReturn } from "../../credits/returns";
import CategoryFlap from "../../discovery/components/CategoryFlap";
import Icon from "../../discovery/components/Icon";
import { useCreatorName } from "../../discovery/hooks";
import { rarityCardPalette } from "../../discovery/lib/rarity";
import { myItemsRouteFor } from "../../discovery/lib/routes";
import { Confetti, JumpInIcon } from "../../history/celebration";
import { creditsCeil, formatMana } from "../../model/money";
import type { MixedPayment } from "../../payment/present";
import { t } from "../../discovery/intl";
import type { CartLineDisplay } from "../../model/purchase";
import { SHOP_BASE } from "../../routes";
import { useShopWallet } from "../../wallet/context";
import { backpackDeepLink, jumpUrl } from "../lib/jump";
import buyErrorArt from "../../checkout/assets/buy-error.png?url";
import packCoin from "../../checkout/assets/pack-coin.webp?url";
import styles from "../../checkout/components/CheckoutModal.module.css";

const TEXT = {
  titleBuy: "Buy Item",
  titleNoFunds: "Buy Credits and Item",
  titleComplete: "Purchase complete!",
  titleError: "Oops! Something went wrong",
  balance: "My Credits Balance:",
  close: "Close",
  insufficient: "Insufficient Funds.",
  needToBuy: "You will need to buy",
  toPurchase: "to purchase this item.",
  learnMore: "Learn More",
  cancel: "Cancel",
  buy: "Buy",
  choosePayment: "Choose your payment method",
  completing: "Completing your purchase…",
  completingResume: "Completing Purchase…",
  stillConfirming: "Your purchase is still being confirmed. It can take a few minutes; nothing is charged twice.",
  checkAgain: "Check again",
  successTitle: "Your purchase was successful!",
  successBody: "You can find your item in the My Items tab.",
  myItems: "My items",
  tryInWorld: "Try in world",
  goToBackpack: "Go to backpack",
  errorHeadline: "We couldn't complete your purchase this time.",
  errorBody: "Don't worry, your Credits are safe. Please try again in a moment.",
  outage: "Prices can't be checked right now. Try again in a moment.",
  tryAgain: "Try again",
  itemFallback: "Item",
  priceChanged: "The price for this item changed while you were here. Check the new amount and confirm to continue.",
} as const;

/** One namespace per item in this tab's session storage, so each dialog's cart and checkout are its own. */
function sessionStorageFor(namespace: string): () => CartStorage | null {
  return () => {
    let s: Storage;
    try {
      if (typeof sessionStorage === "undefined") return null;
      s = sessionStorage;
    } catch {
      return null;
    }
    return {
      getItem: (k) => s.getItem(`${namespace}${k}`),
      setItem: (k, v) => s.setItem(`${namespace}${k}`, v),
      removeItem: (k) => s.removeItem(`${namespace}${k}`),
    };
  };
}

// The dialogs' carts, one per account and item for this tab. A store of purchase state, not a cache of server state: the
// same kind as the cart's own module-level store (cart/store.ts shopCart), which docs/web-app-standards.md allows.
const stores = new Map<string, CartStore>();

const storeKey = (account: string | null, lineId: string) => `${account?.toLowerCase() ?? "anonymous"}:${lineId}`;
const settlingIn = (cart: CartStore) =>
  !!cart.getSnapshot().checkout?.groups.some((g) => g.status === "authorizing" || g.status === "submitted" || g.status === "pending");

/**
 * The dialog's cart for this account and item: created once and kept while it holds a purchase in flight, so that one
 * survives the dialog (and, through session storage, a reload).
 */
function buyNowCart(account: string | null, lineId: string): CartStore {
  const key = storeKey(account, lineId);
  let cart = stores.get(key);
  if (!cart) {
    cart = createCartStore({ storage: sessionStorageFor(`dcl-shop-buynow:v1:${lineId}:`) });
    cart.scopeTo(account);
    stores.set(key, cart);
  }
  return cart;
}

/** The line goes in once however often the dialog renders or opens; a held one takes the offer the page now shows. */
function holdLine(cart: CartStore, line: CartAddInput): void {
  const id = cartLineId(line.asset, line.listing);
  const held = cart.getSnapshot().lines.some((l) => l.id === id);
  if (!held) cart.add({ ...line, quantity: 1 });
  else if (!settlingIn(cart) && line.seen) cart.markSeen(id, line.seen);
}

/** A closed dialog's cart is let go unless a purchase in it is still settling (its storage keeps it either way). */
function releaseBuyNowCart(account: string | null, lineId: string): void {
  const key = storeKey(account, lineId);
  const cart = stores.get(key);
  if (cart && !settlingIn(cart)) stores.delete(key);
}

/** For tests: forget every dialog's cart. */
export function resetBuyNowCarts(): void {
  stores.clear();
}

export type BuyModalProps = {
  /** The item as the page adds it to a cart: its listing, what it shows, the offer the buyer saw. */
  line: CartAddInput;
  /** The detail page to come back to from a credits top-up, which then finishes the purchase. */
  returnPath: string;
  /** Back from that top-up with its record: the account, the line and the Credits price the buyer confirmed. */
  resume?: Pick<BuyNowReturn, "account" | "lineId" | "credits"> | null;
  /** The dialog took the resume (paid with it, or set it aside for a press): the page drops it, so no remount reuses it. */
  onResumeUsed?(): void;
  /**
   * The buyer's Credits first and MANA for the rest, in one transaction (CombinedBuy): the page's own flow, which the
   * dialog hands the split to until upstream's payment-method step (Credits + MANA ticked together) is ported.
   */
  onBuyMixed?(mixed: MixedPayment): void;
  onClose(): void;
};

export default function BuyModal(props: BuyModalProps) {
  const { account } = useShopWallet();
  const lineId = cartLineId(props.line.asset, props.line.listing);
  // Per account and line, not per render: the page hands a fresh line object on every render.
  const cart = useMemo(() => buyNowCart(account, lineId), [account, lineId]);
  const lineRef = useRef(props.line);
  lineRef.current = props.line;
  useEffect(() => holdLine(cart, lineRef.current), [cart]);
  const services = useMemo(() => ({ cart }), [cart]);
  // However the dialog goes (closed, the split taken to the page, the page left), its cart is let go once nothing in it
  // is settling.
  useEffect(() => () => releaseBuyNowCart(account, lineId), [account, lineId]);
  if (typeof document === "undefined") return null;
  return createPortal(
    <EnsureQueryClient>
      <ShopServicesProvider value={services}>
        <BuyDialog key={account ?? "anonymous"} {...props} />
      </ShopServicesProvider>
    </EnsureQueryClient>,
    document.body,
  );
}

type Phase = "loading" | "nofunds" | "ready" | "processing" | "pending" | "complete" | "error";

function BuyDialog({ line, returnPath, resume = null, onResumeUsed, onBuyMixed, onClose }: BuyModalProps) {
  const navigate = useNavigate();
  const services = useShopServices();
  const [purchased, setPurchased] = useState<PurchasedLine[] | null>(null);
  // The resume confirms on its own (below), once and only at the agreed price: never the hook's standing auto-confirm.
  const co = useCheckout({ onComplete: (lines) => setPurchased(lines), claimsCartResume: false });
  const display: CartLineDisplay = line.display;

  const cartLine = co.lines[0] ?? null;
  const view = cartLine ? (co.views[cartLine.id] ?? null) : null;
  const credits = view?.options.find((o) => o.method === "credits") ?? null;
  const mana = view?.options.find((o) => o.method === "mana") ?? null;
  const results = Object.values(co.results);
  const result = results[0] ?? null;
  const creditsQuote = credits?.quote?.method === "credits" ? credits.quote : null;
  const priceCredits = creditsQuote ? creditsCeil(creditsQuote.quoted) : (display.priceCredits ?? 0);
  // A buyer holding MANA chooses how to pay, as upstream's method step: MANA alone may cover what the Credits do not.
  const holdsMana = co.session.kind === "web3" && (co.manaBalance ?? 0n) > 0n && !!mana;
  const manaPays = holdsMana && mana?.state.kind === "ready";
  const creditsShort = credits?.state.kind === "topUpRequired" ? creditsCeil(credits.state.short) : null;

  // A price the review found changed since the page, or one above what the buyer confirmed before a top-up: shown, and
  // agreed to by pressing Buy.
  const resumeRepriced = !!resume && priceCredits > resume.credits;
  const reviewRepriced = co.reviewChanges.some((c) => c.kind === "priceChanged");
  const priceChanged = reviewRepriced || resumeRepriced;
  const settling = !!co.summary && co.summary.submitted + co.summary.pending > 0;
  const confirmed = results.length > 0 && results.every((r) => r.status === "confirmed");
  // No price to charge: the whole check failed, or the Credits price is out and no MANA the buyer holds can pay instead.
  const outage = co.review.kind === "outage" || (credits?.state.kind === "unavailable" && !manaPays);
  // Loading is the first check only: a re-price after the buyer picks how to pay keeps the screen, with Buy held until
  // the new price is in (the checkout's own blocker).
  const [priced, setPriced] = useState(false);
  const checked = !!view && co.review.kind !== "checking";
  useEffect(() => {
    if (checked) setPriced(true);
  }, [checked]);

  const phase: Phase =
    purchased || confirmed
      ? "complete"
      : co.running
        ? "processing"
        : settling || result?.status === "pending"
          ? "pending"
          : result?.status === "failed" && !result.topUp
            ? "error"
            : outage
              ? "error"
              : !view || (!priced && !checked) || (co.balance === undefined && !co.balanceError)
                ? "loading"
                : (creditsShort !== null || co.topUp || result?.status === "failed") && !manaPays
                  ? "nofunds"
                  : "ready";

  const busy = phase === "processing";
  // Closing drops this dialog's top-up hand-off: a top-up finished later must not come back and buy this item. So does
  // any other way the dialog goes, except leaving for the pack checkout it started.
  const attempt = useRef<string | null>(null);
  const leaving = useRef(false);
  useEffect(
    () => () => {
      if (attempt.current && !leaving.current) dropBuyNowTopUp(attempt.current);
    },
    [],
  );
  const close = () => {
    if (busy) return;
    if (attempt.current) {
      dropBuyNowTopUp(attempt.current);
      attempt.current = null;
    }
    onClose();
  };
  const closeRef = useRef(close);
  closeRef.current = close;
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") closeRef.current();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  // Leaving the page while the purchase is being sent would leave it unwatched: the browser asks first.
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
  const dialogRef = useRef<HTMLDivElement>(null);
  // Focus moves into the dialog, and back to what had it (Buy now) when the dialog goes.
  useEffect(() => {
    const before = typeof document !== "undefined" ? (document.activeElement as HTMLElement | null) : null;
    dialogRef.current?.focus();
    return () => before?.focus?.();
  }, []);
  // A modal dialog keeps Tab and Shift+Tab inside it.
  const trapTab = (e: ReactKeyboardEvent) => {
    if (e.key !== "Tab" || !dialogRef.current) return;
    const focusable = [...dialogRef.current.querySelectorAll<HTMLElement>('a[href], button:not([disabled]), input:not([disabled]), [tabindex]:not([tabindex="-1"])')];
    if (focusable.length === 0) return;
    const first = focusable[0]!;
    const last = focusable[focusable.length - 1]!;
    if (e.shiftKey && (document.activeElement === first || document.activeElement === dialogRef.current)) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  };

  // Back from a top-up: the buyer already confirmed, so the purchase goes on as soon as it is priced, once, if it is the
  // same line at no more than the price they agreed to. Anything else waits for a press.
  const resumed = useRef(false);
  useEffect(() => {
    if (!resume || resumed.current || phase !== "ready" || !cartLine || !co.canConfirm) return;
    resumed.current = true;
    onResumeUsed?.();
    if (resume.account !== co.account?.toLowerCase() || cartLine.id !== resume.lineId || priceChanged || cartLine.method) return;
    void co.confirm();
  }, [resume, phase, cartLine, co, priceChanged, onResumeUsed]);

  const selected = cartLine?.method ?? view?.primary ?? "credits";
  const [useMixed, setUseMixed] = useState(false);
  const mixed = selected === "mana" && onBuyMixed ? (mana?.mixed ?? null) : null;
  const pay = () => {
    if (mixed && useMixed) {
      onBuyMixed!(mixed);
      return;
    }
    if (reviewRepriced) {
      // The new price is on screen now; this press agrees to it, and the next one pays.
      co.acceptChanges();
      return;
    }
    void co.confirm();
  };

  if (co.approval) {
    return (
      <AuthorizeStep
        auth={co.approval.request.auth}
        step={{ index: co.approval.request.index, total: co.approval.request.total }}
        requiredWei={co.approval.request.requiredWei}
        grant={co.approval.grant}
        onAuthorized={() => co.approval?.resolve(true)}
        onCancel={() => co.approval?.resolve(false)}
        portal={false}
      />
    );
  }

  const title = phase === "complete" ? TEXT.titleComplete : phase === "error" ? TEXT.titleError : phase === "nofunds" ? TEXT.titleNoFunds : TEXT.titleBuy;
  const balance = co.balanceError ? "—" : (co.balance?.credits ?? 0).toLocaleString("en-US");
  return (
    <div className={styles.root} role="dialog" aria-modal="true" aria-label={`Buy ${display.name || TEXT.itemFallback}`} data-testid="buy-modal" ref={dialogRef} tabIndex={-1} onKeyDown={trapTab}>
      <div className={styles.scrim} onClick={busy ? undefined : close} aria-hidden="true" data-testid="buy-modal-scrim" />
      <div className={styles.card} data-tall={phase === "processing" || phase === "loading" || undefined} data-phase={phase}>
        <div className={styles.head}>
          <div className={styles.headrow}>
            <h2 className={styles.title}>{title}</h2>
            {!busy ? (
              <button type="button" className={styles.x} onClick={close} aria-label={TEXT.close} data-testid="buy-modal-close">
                <svg viewBox="0 0 18 18" width="18" height="18" aria-hidden="true">
                  <path d="M4 4l10 10M14 4L4 14" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
                </svg>
              </button>
            ) : null}
          </div>
          <div className={styles.balance} data-testid="buy-modal-balance">
            <span className={styles.balanceLabel}>{TEXT.balance}</span>
            <Icon name="credits" className={styles.balanceIco} />
            <span className={styles.balanceValue}>{balance}</span>
          </div>
        </div>

        {phase === "loading" ? (
          <div className={styles.body} data-processing>
            <span className={styles.spinner} aria-hidden="true" />
          </div>
        ) : null}

        {phase === "error" ? (
          <ErrorBody
            message={outage ? TEXT.outage : result?.status === "failed" ? result.failure.message : null}
            onClose={close}
            onRetry={() => {
              if (outage) co.revalidate();
              else void co.discardCheckout();
            }}
          />
        ) : null}

        {phase === "nofunds" ? (
          <NoFunds
            display={display}
            priceCredits={priceCredits}
            shortfall={Math.max(1, creditsShort ?? (co.topUp ? creditsCeil(co.topUp.shortCents) : priceCredits - (co.balance?.credits ?? 0)))}
            onLeave={() => {
              if (!co.account || !cartLine) return null;
              attempt.current = stashBuyNowTopUp({ path: returnPath, account: co.account, lineId: cartLine.id, credits: priceCredits });
              return attempt.current;
            }}
            onFailed={(failed) => {
              if (!failed) return;
              dropBuyNowTopUp(failed);
              if (attempt.current === failed) attempt.current = null;
            }}
            onCancel={close}
            onCheckout={(session) => {
              // Only this order's grant may bring the buyer back to finish: an abandoned top-up never is.
              if (attempt.current) bindBuyNowTopUp(attempt.current, session.orderId);
              leaving.current = true;
              if (session.url) services.openExternal(session.url);
              else navigate(`${SHOP_BASE}/credits?order=${encodeURIComponent(session.orderId)}`);
            }}
          />
        ) : null}

        {phase === "ready" ? (
          <div className={styles.body}>
            {priceChanged ? (
              <div className={styles.warning} role="status" data-testid="price-changed">
                <WarningIcon />
                <p className={styles.warningText}>{TEXT.priceChanged}</p>
              </div>
            ) : null}
            {holdsMana && view && cartLine ? (
              <section aria-label={TEXT.choosePayment} data-testid="buy-modal-methods">
                <PaymentChoice
                  view={view}
                  selected={selected}
                  // The line's own method: the checkout prices, partitions and pays what is chosen here.
                  onSelect={(m) => co.setMethod(cartLine.id, m === view.primary ? null : m)}
                  quantity={1}
                  manaChainId={cartLine.asset.chainId}
                  label={display.name}
                  mixedNote={false}
                />
                {mixed ? (
                  <label className={styles.warningText} data-testid="buy-modal-mixed">
                    <input type="checkbox" checked={useMixed} onChange={(e) => setUseMixed(e.target.checked)} />{" "}
                    {t("itemDetail.mixedOption", { credits: t("assetCard.credits", { count: mixed.credits }), mana: formatMana(mixed.manaWei, { group: true }) })}
                  </label>
                ) : null}
              </section>
            ) : null}
            <AssetRow display={display} priceCredits={priceCredits} />
            <div className={styles.ctas}>
              <button
                type="button"
                className={styles.btn}
                data-variant="gradient"
                data-full
                onClick={pay}
                disabled={!co.canConfirm && !reviewRepriced && !(mixed && useMixed)}
                data-testid="buy-modal-buy"
              >
                {TEXT.buy}
              </button>
            </div>
          </div>
        ) : null}

        {phase === "processing" ? (
          <div className={styles.body} data-processing>
            <img className={styles.logo} src={loaderLogo} alt="" width={61} height={61} />
            <div className={styles.processing}>{resume ? TEXT.completingResume : TEXT.completing}</div>
            <div className={styles.progress} aria-hidden="true">
              <span className={styles.progressFill} />
            </div>
          </div>
        ) : null}

        {phase === "pending" ? (
          <div className={styles.body} data-testid="buy-modal-pending">
            <div className={styles.success}>
              <img className={styles.logo} src={loaderLogo} alt="" width={61} height={61} />
              <p className={styles.processing}>{TEXT.stillConfirming}</p>
            </div>
            <div className={styles.ctas}>
              <button type="button" className={styles.btn} data-variant="outline" onClick={close}>
                {TEXT.close}
              </button>
              <button type="button" className={styles.btn} data-variant="ruby" onClick={() => void co.resume()} disabled={co.running}>
                {TEXT.checkAgain}
              </button>
            </div>
          </div>
        ) : null}

        {phase === "complete" ? (
          <div className={styles.body}>
            <Confetti className={styles.confetti} />
            <div className={styles.success} data-testid="buy-modal-complete">
              <svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true">
                <circle cx="32" cy="32" r="32" fill="#34ce74" />
                <path d="M20 33l8 8 16-18" fill="none" stroke="#fff" strokeWidth="5" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
              <p className={styles.successText}>
                <b>{TEXT.successTitle}</b> {TEXT.successBody}
              </p>
            </div>
            <div className={styles.ctas} data-stack>
              <button type="button" className={styles.btn} data-variant="outline" onClick={() => navigate(myItemsRouteFor([display.category]))}>
                {TEXT.myItems}
              </button>
              {isIapMode() ? (
                <a className={styles.btn} data-variant="ruby" href={backpackDeepLink(purchased ?? [{ urn: display.urn }])} onClick={onClose}>
                  {TEXT.goToBackpack}
                </a>
              ) : (
                <a className={styles.btn} data-variant="ruby" href={jumpUrl()} target="_blank" rel="noreferrer" onClick={onClose}>
                  {TEXT.tryInWorld}
                  <JumpInIcon />
                </a>
              )}
            </div>
          </div>
        ) : null}
      </div>
    </div>
  );
}

/** The item: its artwork on its rarity's tile with the category flap, its name, its creator and its price. */
function AssetRow({ display, priceCredits }: { display: CartLineDisplay; priceCredits: number }) {
  const creator = useCreatorName(display.creator);
  const tile: CSSProperties = { background: rarityCardPalette(display.rarity).background };
  return (
    <div className={styles.asset} data-testid="buy-modal-asset">
      <div className={styles.thumb} style={tile}>
        <CategoryFlap item={{ category: display.category ?? "wearable", wearableCategory: display.wearableCategory, rarity: display.rarity ?? "" }} size="sm" testId="buy-modal-flap" />
        {display.thumbnail ? <img src={display.thumbnail} alt="" /> : null}
      </div>
      <div className={styles.info}>
        <div>
          <div className={styles.name} title={display.name}>
            {display.name || TEXT.itemFallback}
          </div>
          {display.creator ? <div className={styles.creator}>{creator ? `By ${creator}` : " "}</div> : null}
        </div>
        <div className={styles.price}>
          <Icon name="credits" className={styles.priceIco} />
          <span>{priceCredits.toLocaleString("en-US")}</span>
        </div>
      </div>
    </div>
  );
}

const WarningIcon = () => (
  <svg viewBox="0 0 20 20" width="24" height="24" aria-hidden="true">
    <path
      fill="#f48221"
      d="M10 0a10 10 0 1 1 0 20 10 10 0 0 1 0-20Zm0 2a8 8 0 1 0 0 16 8 8 0 0 0 0-16Zm0 11a1 1 0 1 1 0 2 1 1 0 0 1 0-2Zm0-8a1 1 0 0 1 1 1v5a1 1 0 1 1-2 0V6a1 1 0 0 1 1-1Z"
    />
  </svg>
);

/**
 * Not enough Credits and no MANA that can pay: what the buyer is short, the item, and the packs that finish the purchase
 * (all of them when none does), the cheapest of those picked. Buy leaves for the pack checkout with the hand-off that
 * brings the buyer back to finish; a dialog closed while the checkout is being created never leaves.
 */
function NoFunds(props: {
  display: CartLineDisplay;
  priceCredits: number;
  shortfall: number;
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
      setError(friendlyError(e, "Couldn't start the Credits checkout — please try again."));
      setBusy(false);
    }
  }

  return (
    <div className={styles.body}>
      <div className={styles.warning} data-testid="buy-modal-insufficient">
        <WarningIcon />
        <p className={styles.warningText}>
          <b>{TEXT.insufficient}</b> {TEXT.needToBuy} <b>{`${props.shortfall.toLocaleString("en-US")} Credits`}</b> {TEXT.toPurchase}
          {iap ? null : (
            <>
              {" "}
              <a className={styles.warningLink} href={`${SHOP_BASE}/credits`} target="_blank" rel="noopener noreferrer">
                {TEXT.learnMore}
              </a>
            </>
          )}
        </p>
      </div>
      <AssetRow display={props.display} priceCredits={props.priceCredits} />
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
          <button type="button" className={styles.btn} data-variant="gradient" onClick={() => void buy()} disabled={busy || !selectedId} data-testid="buy-modal-topup">
            {TEXT.buy}
          </button>
        )}
      </div>
    </div>
  );
}

/** The cart's failure panel: the reason, that the Credits are safe, and a retry. */
function ErrorBody({ message, onClose, onRetry }: { message: string | null; onClose(): void; onRetry(): void }) {
  return (
    <div className={styles.body}>
      <div className={styles.error} data-testid="buy-error">
        <img className={styles.errorArt} src={buyErrorArt} alt="" width={64} height={80} />
        <p className={styles.errorText}>
          <b>{TEXT.errorHeadline}</b> {message ?? TEXT.errorBody}
        </p>
      </div>
      <div className={styles.ctas}>
        <button type="button" className={styles.btn} data-variant="outline" onClick={onClose}>
          {TEXT.cancel}
        </button>
        <button type="button" className={styles.btn} data-variant="ruby" onClick={onRetry}>
          {TEXT.tryAgain}
        </button>
      </div>
    </div>
  );
}
