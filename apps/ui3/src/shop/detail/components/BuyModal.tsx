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
//     (../buyNowReturn.ts), the dialog finishes on its own, at that price or less;
//   - "Buy Item" when the Credits cover it; a buyer holding MANA chooses how to pay first;
//   - the approval a self-custody wallet grants, the progress, a purchase still being confirmed, "Purchase complete!"
//     and the error.
import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
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
import { clearTopUpHandoffs, stashTopUpReturn } from "../../credits/returns";
import CategoryFlap from "../../discovery/components/CategoryFlap";
import Icon from "../../discovery/components/Icon";
import { useCreatorName } from "../../discovery/hooks";
import { rarityCardPalette } from "../../discovery/lib/rarity";
import { myItemsRouteFor } from "../../discovery/lib/routes";
import { Confetti, JumpInIcon } from "../../history/celebration";
import { creditsCeil } from "../../model/money";
import type { CartLineDisplay } from "../../model/purchase";
import { SHOP_BASE } from "../../routes";
import { useShopWallet } from "../../wallet/context";
import { clearBuyNowReturn, stashBuyNowReturn } from "../buyNowReturn";
import { backpackDeepLink, jumpUrl } from "../lib/jump";
import buyErrorArt from "../../checkout/assets/buy-error.png?url";
import packCoin from "../../checkout/assets/pack-coin.webp?url";
import "../../checkout/components/checkoutmodal.css";

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

const stores = new Map<string, CartStore>();

/**
 * The dialog's cart for this account and item: created once and kept, so a purchase in flight survives the dialog (and,
 * through session storage, a reload), and the line is added once however often the dialog renders or opens.
 */
function buyNowCart(account: string | null, line: CartAddInput): CartStore {
  const id = cartLineId(line.asset, line.listing);
  const key = `${account ?? "anonymous"}:${id}`;
  let cart = stores.get(key);
  if (!cart) {
    cart = createCartStore({ storage: sessionStorageFor(`dcl-shop-buynow:v1:${id}:`) });
    cart.scopeTo(account);
    stores.set(key, cart);
  }
  const snap = cart.getSnapshot();
  const held = snap.lines.find((l) => l.id === id);
  const settling = !!snap.checkout?.groups.some((g) => g.status === "authorizing" || g.status === "submitted" || g.status === "pending");
  if (!held) cart.add({ ...line, quantity: 1 });
  else if (!settling) cart.markSeen(id, line.seen ?? null);
  return cart;
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
  /** Back from that top-up with its record: the line and the Credits price the buyer confirmed. */
  resume?: { lineId: string; credits: number } | null;
  onClose(): void;
};

export default function BuyModal(props: BuyModalProps) {
  const { account } = useShopWallet();
  const lineId = cartLineId(props.line.asset, props.line.listing);
  // Per account and line, not per render: the page hands a fresh line object on every render.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const cart = useMemo(() => buyNowCart(account, props.line), [account, lineId]);
  const services = useMemo(() => ({ cart }), [cart]);
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

function BuyDialog({ line, returnPath, resume = null, onClose }: BuyModalProps) {
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
  const priceChanged = co.reviewChanges.some((c) => c.kind === "priceChanged") || resumeRepriced;
  const settling = !!co.summary && co.summary.submitted + co.summary.pending > 0;
  const confirmed = results.length > 0 && results.every((r) => r.status === "confirmed");
  // No price to charge: the whole check failed, or the Credits price is out and no MANA the buyer holds can pay instead.
  const outage = co.review.kind === "outage" || (credits?.state.kind === "unavailable" && !manaPays);
  // Loading is the first check only: a re-price after the buyer picks how to pay keeps the screen, with Buy held until
  // the new price is in (the checkout's own blocker).
  const priced = useRef(false);
  if (view && co.review.kind !== "checking") priced.current = true;

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
              : !view || (!priced.current && co.review.kind === "checking") || (co.balance === undefined && !co.balanceError)
                ? "loading"
                : (creditsShort !== null || co.topUp || result?.status === "failed") && !manaPays
                  ? "nofunds"
                  : "ready";

  const busy = phase === "processing";
  // Closing drops this dialog's top-up hand-off: a top-up finished later must not come back and buy this item.
  const stashed = useRef(false);
  const close = () => {
    if (busy) return;
    if (stashed.current) {
      clearTopUpHandoffs();
      clearBuyNowReturn();
      stashed.current = false;
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
    const hold = (e: BeforeUnloadEvent) => e.preventDefault();
    window.addEventListener("beforeunload", hold);
    return () => window.removeEventListener("beforeunload", hold);
  }, [busy]);
  const dialogRef = useRef<HTMLDivElement>(null);
  useEffect(() => dialogRef.current?.focus(), []);

  // Back from a top-up: the buyer already confirmed, so the purchase goes on as soon as it is priced, once, if it is the
  // same line at no more than the price they agreed to. Anything else waits for a press.
  const resumed = useRef(false);
  useEffect(() => {
    if (!resume || resumed.current || phase !== "ready" || !cartLine || !co.canConfirm) return;
    resumed.current = true;
    if (cartLine.id !== resume.lineId || priceChanged || cartLine.method) return;
    void co.confirm();
  }, [resume, phase, cartLine, co, priceChanged]);

  const selected = cartLine?.method ?? view?.primary ?? "credits";
  const pay = () => {
    if (co.reviewChanges.some((c) => c.kind === "priceChanged")) {
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
    <div className="shop-ckm" role="dialog" aria-modal="true" aria-label={`Buy ${display.name || TEXT.itemFallback}`} data-testid="buy-modal" ref={dialogRef} tabIndex={-1}>
      <div className="shop-ckm__scrim" onClick={busy ? undefined : close} aria-hidden="true" />
      <div className="shop-ckm__card" data-surface="dark" data-tall={phase === "processing" || phase === "loading" || undefined} data-phase={phase}>
        <div className="shop-ckm__head">
          <div className="shop-ckm__headrow">
            <h2 className="shop-ckm__title">{title}</h2>
            {!busy ? (
              <button type="button" className="shop-ckm__x" onClick={close} aria-label={TEXT.close} data-testid="buy-modal-close">
                <svg viewBox="0 0 18 18" width="18" height="18" aria-hidden="true">
                  <path d="M4 4l10 10M14 4L4 14" stroke="#161518" strokeWidth="1.8" strokeLinecap="round" />
                </svg>
              </button>
            ) : null}
          </div>
          <div className="shop-ckm__balance" data-testid="buy-modal-balance">
            <span className="shop-ckm__balance-label">{TEXT.balance}</span>
            <Icon name="credits" className="shop-ckm__balance-ico" />
            <span className="shop-ckm__balance-value">{balance}</span>
          </div>
        </div>

        {phase === "loading" ? (
          <div className="shop-ckm__body" data-processing>
            <span className="shop-ckm__spinner" aria-hidden="true" />
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
              stashTopUpReturn(`${returnPath}?buy=resume`);
              if (cartLine) stashBuyNowReturn({ path: returnPath, lineId: cartLine.id, credits: priceCredits });
              stashed.current = true;
            }}
            onFailed={() => {
              clearTopUpHandoffs();
              clearBuyNowReturn();
              stashed.current = false;
            }}
            onCancel={close}
            onCheckout={(session) => (session.url ? services.openExternal(session.url) : navigate(`${SHOP_BASE}/credits?order=${encodeURIComponent(session.orderId)}`))}
          />
        ) : null}

        {phase === "ready" ? (
          <div className="shop-ckm__body">
            {priceChanged ? (
              <div className="shop-ckm__warning" role="status" data-testid="price-changed">
                <WarningIcon />
                <p className="shop-ckm__warning-text">{TEXT.priceChanged}</p>
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
              </section>
            ) : null}
            <AssetRow display={display} priceCredits={priceCredits} />
            <div className="shop-ckm__ctas">
              <button
                type="button"
                className="shop-ckm__btn"
                data-variant="gradient"
                data-full
                onClick={pay}
                disabled={!co.canConfirm && !co.reviewChanges.some((c) => c.kind === "priceChanged")}
                data-testid="buy-modal-buy"
              >
                {TEXT.buy}
              </button>
            </div>
          </div>
        ) : null}

        {phase === "processing" ? (
          <div className="shop-ckm__body" data-processing>
            <img className="shop-ckm__logo" src={loaderLogo} alt="" width={61} height={61} />
            <div className="shop-ckm__processing">{resume ? TEXT.completingResume : TEXT.completing}</div>
            <div className="shop-ckm__progress" aria-hidden="true">
              <span className="shop-ckm__progress-fill" />
            </div>
          </div>
        ) : null}

        {phase === "pending" ? (
          <div className="shop-ckm__body" data-testid="buy-modal-pending">
            <div className="shop-ckm__success">
              <img className="shop-ckm__logo" src={loaderLogo} alt="" width={61} height={61} />
              <p className="shop-ckm__processing">{TEXT.stillConfirming}</p>
            </div>
            <div className="shop-ckm__ctas">
              <button type="button" className="shop-ckm__btn" data-variant="outline" onClick={close}>
                {TEXT.close}
              </button>
              <button type="button" className="shop-ckm__btn" data-variant="ruby" onClick={() => void co.resume()} disabled={co.running}>
                {TEXT.checkAgain}
              </button>
            </div>
          </div>
        ) : null}

        {phase === "complete" ? (
          <div className="shop-ckm__body">
            <Confetti className="shop-ckm__confetti" />
            <div className="shop-ckm__success" data-testid="buy-modal-complete">
              <svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true">
                <circle cx="32" cy="32" r="32" fill="#34ce74" />
                <path d="M20 33l8 8 16-18" fill="none" stroke="#fff" strokeWidth="5" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
              <p className="shop-ckm__success-text">
                <b>{TEXT.successTitle}</b> {TEXT.successBody}
              </p>
            </div>
            <div className="shop-ckm__ctas" data-stack>
              <button type="button" className="shop-ckm__btn" data-variant="outline" onClick={() => navigate(myItemsRouteFor([display.category]))}>
                {TEXT.myItems}
              </button>
              {isIapMode() ? (
                <a className="shop-ckm__btn" data-variant="ruby" href={backpackDeepLink(purchased ?? [{ urn: display.urn }])} onClick={onClose}>
                  {TEXT.goToBackpack}
                </a>
              ) : (
                <a className="shop-ckm__btn" data-variant="ruby" href={jumpUrl()} target="_blank" rel="noreferrer" onClick={onClose}>
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
    <div className="shop-ckm__asset" data-testid="buy-modal-asset">
      <div className="shop-ckm__thumb" style={tile}>
        <CategoryFlap item={{ category: display.category ?? "wearable", wearableCategory: display.wearableCategory, rarity: display.rarity ?? "" }} size="sm" testId="buy-modal-flap" />
        {display.thumbnail ? <img src={display.thumbnail} alt="" /> : null}
      </div>
      <div className="shop-ckm__info">
        <div>
          <div className="shop-ckm__name" title={display.name}>
            {display.name || TEXT.itemFallback}
          </div>
          {display.creator ? <div className="shop-ckm__creator">{creator ? `By ${creator}` : " "}</div> : null}
        </div>
        <div className="shop-ckm__price">
          <Icon name="credits" className="shop-ckm__price-ico" />
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
  onLeave(): void;
  onFailed(): void;
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
  const mounted = useRef(true);
  useEffect(
    () => () => {
      mounted.current = false;
    },
    [],
  );
  const iap = isIapMode();

  async function buy() {
    if (!selectedId) return;
    setBusy(true);
    setError(null);
    props.onLeave();
    try {
      const session = await createPackCheckout(selectedId, client);
      if (!mounted.current) {
        props.onFailed();
        return;
      }
      props.onCheckout(session);
    } catch (e) {
      props.onFailed();
      if (!mounted.current) return;
      setError(friendlyError(e, "Couldn't start the Credits checkout — please try again."));
      setBusy(false);
    }
  }

  return (
    <div className="shop-ckm__body">
      <div className="shop-ckm__warning" data-testid="buy-modal-insufficient">
        <WarningIcon />
        <p className="shop-ckm__warning-text">
          <b>{TEXT.insufficient}</b> {TEXT.needToBuy} <b>{`${props.shortfall.toLocaleString("en-US")} Credits`}</b> {TEXT.toPurchase}
          {iap ? null : (
            <>
              {" "}
              <a className="shop-ckm__warning-link" href={`${SHOP_BASE}/credits`} target="_blank" rel="noopener noreferrer">
                {TEXT.learnMore}
              </a>
            </>
          )}
        </p>
      </div>
      <AssetRow display={props.display} priceCredits={props.priceCredits} />
      {iap ? null : (
        <>
          <div className="shop-ckm__packs" data-testid="credit-packs">
            {offer.packs.map((p) => (
              <button
                key={p.id}
                type="button"
                className="shop-ckm__pack"
                data-testid="credit-pack"
                data-on={p.id === selectedId || undefined}
                aria-pressed={p.id === selectedId}
                aria-label={`${p.credits.toLocaleString("en-US")} Credits for $${p.usd.toFixed(2)}`}
                onClick={() => setPicked(p.id)}
              >
                <img className="shop-ckm__pack-ico" src={packCoin} alt="" />
                <span className="shop-ckm__pack-amount">{p.credits.toLocaleString("en-US")}</span>
                <span className="shop-ckm__pack-usd">(${p.usd.toFixed(2)})</span>
              </button>
            ))}
          </div>
          <div className="shop-ckm__total">
            <div className="shop-ckm__total-credits">
              <Icon name="credits" className="shop-ckm__total-ico" />
              <span data-testid="topup-total-credits">{(selected?.credits ?? 0).toLocaleString("en-US")}</span>
            </div>
            <span className="shop-ckm__total-usd">${(selected?.usd ?? 0).toFixed(2)}</span>
          </div>
        </>
      )}
      {error ? (
        <p className="shop-ckm__warning-text" role="alert">
          {error}
        </p>
      ) : null}
      <div className="shop-ckm__ctas">
        <button type="button" className="shop-ckm__btn" data-variant="outline" onClick={props.onCancel}>
          {TEXT.cancel}
        </button>
        {iap ? null : (
          <button type="button" className="shop-ckm__btn" data-variant="gradient" onClick={() => void buy()} disabled={busy || !selectedId} data-testid="buy-modal-topup">
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
    <div className="shop-ckm__body">
      <div className="shop-ckm__error" data-testid="buy-error">
        <img className="shop-ckm__error-art" src={buyErrorArt} alt="" width={64} height={80} />
        <p className="shop-ckm__error-text">
          <b>{TEXT.errorHeadline}</b> {message ?? TEXT.errorBody}
        </p>
      </div>
      <div className="shop-ckm__ctas">
        <button type="button" className="shop-ckm__btn" data-variant="outline" onClick={onClose}>
          {TEXT.cancel}
        </button>
        <button type="button" className="shop-ckm__btn" data-variant="ruby" onClick={onRetry}>
          {TEXT.tryAgain}
        </button>
      </div>
    </div>
  );
}
