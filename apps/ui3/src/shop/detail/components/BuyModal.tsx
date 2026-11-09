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
import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useNavigate } from "react-router";

import { cartLineId, createCartStore, type CartAddInput, type CartStorage, type CartStore } from "../../cart/store";
import AuthorizeStep from "../../checkout/components/AuthorizeStep";
import {
  DialogAsset,
  DialogBody,
  DialogButton,
  DialogCtas,
  DialogHead,
  DialogLinkButton,
  DialogShell,
  DialogTextButton,
  DialogWarning,
  ErrorBody,
  LoadingBody,
  NoFundsBody,
  PendingBody,
  ProcessingBody,
  useDialog,
} from "../../checkout/components/CheckoutDialog";
import { EnsureQueryClient, ShopServicesProvider, useShopServices } from "../../checkout/services";
import { useCheckout, type Blocker, type BlockerKind, type PurchasedLine } from "../../checkout/useCheckout";
import { isIapMode } from "../../credits/iap";
import { bindBuyNowTopUp, dropBuyNowTopUp, stashBuyNowTopUp, type BuyNowReturn } from "../../credits/returns";
import { myItemsRouteFor } from "../../discovery/lib/routes";
import { invalidateShopQueries } from "../../discovery/query";
import { Confetti, JumpInIcon } from "../../history/celebration";
import { creditsCeil, manaWei, ZERO_MANA } from "../../model/money";
import type { CartLineDisplay } from "../../model/purchase";
import { SHOP_BASE } from "../../routes";
import { useShopWallet } from "../../wallet/context";
import { backpackDeepLink, jumpUrl } from "../lib/jump";
import styles from "./BuyModal.module.css";
import { useCombinedBuy } from "./CombinedBuy";
import PaymentMethodStep, { type StepRail, type StepRails } from "./PaymentMethodStep";

const TEXT = {
  titleBuy: "Buy Item",
  titleNoFunds: "Buy Credits and Item",
  titleComplete: "Purchase complete!",
  titleError: "Oops! Something went wrong",
  close: "Close",
  buy: "Buy",
  completing: "Completing your purchase…",
  completingResume: "Completing Purchase…",
  stillConfirming: "Your purchase is still being confirmed. It can take a few minutes; nothing is charged twice.",
  checkAgain: "Check again",
  connectWallet: "Connect wallet",
  switchAccount: "Sign in with this wallet",
  unbuyable: "This item can't be bought as it is right now.",
  successTitle: "Your purchase was successful!",
  successBody: "You can find your item in the My Items tab.",
  myItems: "My items",
  tryInWorld: "Try in world",
  goToBackpack: "Go to backpack",
  outage: "Prices can't be checked right now. Try again in a moment.",
  gone: "This item is no longer available.",
  itemFallback: "Item",
  priceChanged: "The price for this item changed while you were here. Check the new amount and confirm to continue.",
} as const;

/**
 * The checkout's reasons Buy waits that the dialog shows: the wallet's, with the way back the cart offers, and the
 * item's, in the dialog's own words where the cart's point at controls it does not have ("the note on each item"). The
 * rest have a screen or a notice of their own: the price being checked, a purchase running, the Credits falling short,
 * a new price to accept, prices that cannot be checked.
 */
const HELD: Partial<Record<BlockerKind, { text?: string; action?: "signInWithWallet" | "connectWallet" }>> = {
  accountMismatch: { action: "signInWithWallet" },
  reconnect: { action: "connectWallet" },
  quoteOutage: {},
  offMarket: { text: TEXT.unbuyable },
  itemsChanged: { text: TEXT.unbuyable },
  refused: { text: TEXT.unbuyable },
  methodNotAllowed: { text: TEXT.unbuyable },
};

/** The checkout's reasons about the line itself, which hold for the split too. */
const LINE_BLOCKERS: BlockerKind[] = ["offMarket", "itemsChanged", "refused", "methodNotAllowed", "quoteOutage"];

/**
 * Why the split cannot be bought now, or null. It settles outside the checkout (CombinedBuy), so the checkout's own
 * readiness says nothing of its wallet, but the checkout's word on the line itself (gone, refused, not priced) holds.
 */
export function splitReason(walletBlocked: string | null, accountMismatch: boolean, blocks: readonly Blocker[]): Blocker | null {
  if (walletBlocked) return { kind: accountMismatch ? "accountMismatch" : "reconnect", text: walletBlocked };
  return blocks.find((b) => LINE_BLOCKERS.includes(b.kind)) ?? null;
}

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

// The dialogs' carts, one per account and item for this tab, kept for the tab's life: one let go while a purchase is
// being priced or sent could be created again beside it, unaware of that purchase. A store of purchase state, not a
// cache of server state: the same kind as the cart's own module-level store (cart/store.ts shopCart), which
// docs/web-app-standards.md allows; it holds only the items Buy now was opened on.
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

function BuyDialog({ line, returnPath, resume: given = null, onResumeUsed, onClose }: BuyModalProps) {
  // The resume this dialog opened with, kept for its life: the page drops its own copy once it is used (so a dialog
  // mounted again never reuses it), but this one still owes the buyer the price-change notice it implies.
  const [resume] = useState(given);
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
  // A buyer holding MANA chooses how to pay, as upstream's method step: MANA alone, or the Credits first and MANA for
  // the rest, may cover what the Credits do not.
  const holdsMana = co.session.kind === "web3" && (co.manaBalance ?? 0n) > 0n && !!mana;
  const manaPays = holdsMana && mana?.state.kind === "ready";
  const mixed = holdsMana ? (mana?.mixed ?? null) : null;
  const manaRail = manaPays || !!mixed;
  const priceManaWei = mana?.quote?.method === "mana" ? mana.quote.amount : ZERO_MANA;
  const creditsShort = credits?.state.kind === "topUpRequired" ? creditsCeil(credits.state.short) : null;
  // The split settles in one transaction of its own (CombinedBuy's), shown on this dialog's screens, on the offer the
  // checkout has just priced.
  const combined = useCombinedBuy({ line, offer: cartLine ? (co.offers[cartLine.id] ?? null) : null, mixed, onDone: () => invalidateShopQueries() });
  const run = combined.phase;
  // A split sent earlier and not known to be confirmed (CombinedBuy's record): shown as still confirming, never a second.
  const splitSent = run.kind === "pending" || combined.remembered;

  // A price the review found changed since the page, or one above what the buyer confirmed before a top-up: shown, and
  // agreed to by pressing Buy.
  const resumeRepriced = !!resume && priceCredits > resume.credits;
  const reviewRepriced = co.reviewChanges.some((c) => c.kind === "priceChanged");
  const priceChanged = reviewRepriced || resumeRepriced;
  const settling = !!co.summary && co.summary.submitted + co.summary.pending > 0;
  const confirmed = results.length > 0 && results.every((r) => r.status === "confirmed");
  // No price to charge: the whole check failed, or the Credits price is out and no MANA the buyer holds can pay instead.
  const outage = co.review.kind === "outage" || (credits?.state.kind === "unavailable" && !manaPays);
  // Sold or withdrawn since the page showed it: nothing left to price, which the failure panel says.
  const gone = !!cartLine && co.review.kind !== "checking" && co.reviewChanges.some((c) => c.lineId === cartLine.id && c.kind === "unavailable");
  // Loading is the first check only: a re-price after the buyer picks how to pay keeps the screen, with Buy held until
  // the new price is in (the checkout's own blocker).
  const [priced, setPriced] = useState(false);
  const checked = !!view && co.review.kind !== "checking";
  useEffect(() => {
    if (checked) setPriced(true);
  }, [checked]);

  // The split's approval counts as its purchase running, as the checkout's own does: nothing closes the dialog under it.
  const phase: Phase =
    run.kind === "success"
      ? "complete"
      : run.kind === "working" || run.kind === "approve"
        ? "processing"
        : run.kind === "pending"
          ? "pending"
          : run.kind === "error"
            ? "error"
            : purchased || confirmed
              ? "complete"
              : co.running
                ? "processing"
                : settling || result?.status === "pending" || combined.remembered
                  ? "pending"
                  : result?.status === "failed" && !result.topUp
                    ? "error"
                    : outage || gone
                      ? "error"
                      : !view || (!priced && !checked) || (co.balance === undefined && !co.balanceError)
                        ? "loading"
                        : (creditsShort !== null || co.topUp || result?.status === "failed") && !manaRail
                          ? "nofunds"
                          : "ready";
  // Upstream's method step: for a buyer whose MANA pays (alone or with the Credits), or who holds MANA that does not, so
  // the step says so instead of the option going missing.
  const methodMode = phase === "ready" && (manaRail || (holdsMana && priceManaWei > 0n));

  const busy = phase === "processing";
  // Closing drops this dialog's top-up hand-off: a top-up finished later must not come back and buy this item. So does
  // any other way the dialog goes, except leaving for the pack checkout it started.
  const attempt = useRef<string | null>(null);
  const leaving = useRef(false);
  useEffect(() => {
    // Back from Stripe through the page cache, the buyer did not leave after all: closing again drops the hand-off.
    const onShow = (e: PageTransitionEvent) => {
      if (e.persisted) leaving.current = false;
    };
    window.addEventListener("pageshow", onShow);
    return () => {
      window.removeEventListener("pageshow", onShow);
      if (attempt.current && !leaving.current) dropBuyNowTopUp(attempt.current);
    };
  }, []);
  const close = () => {
    if (busy) return;
    if (attempt.current) {
      dropBuyNowTopUp(attempt.current);
      attempt.current = null;
    }
    onClose();
  };
  // Focus in and back to Buy now, Tab kept inside, Escape, and the browser asking before leaving a purchase being sent.
  const dialog = useDialog(busy, close);
  // After the approval step (drawn in the dialog's place), focus comes back in.
  const approving = !!co.approval || run.kind === "approve";
  useEffect(() => {
    if (!approving) dialog.focus();
  }, [approving, dialog]);

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
  // Why Buy waits (HELD). Not while a new price is what Buy waits for: pressing it agrees to that price. A price that rose
  // since a top-up is only a notice, so a wallet gone in the meantime is still said.
  const held = !co.canConfirm && !reviewRepriced && checked && !co.running ? (co.blocks.find((b) => HELD[b.kind] !== undefined) ?? null) : null;
  const pay = () => {
    if (reviewRepriced) {
      // The new price is on screen now; this press agrees to it, and the next one pays.
      co.acceptChanges();
      return;
    }
    void co.confirm();
  };

  // The method step's pick. Credits or MANA alone is the line's own method, which the checkout prices and pays; the
  // split is CombinedBuy's, and leaves the line as it is.
  const [rail, setRail] = useState<StepRail | null>(null);
  const pick = (next: StepRail | null) => {
    setRail(next);
    if (!cartLine || !view || (next !== "credits" && next !== "mana")) return;
    const method = next === view.primary ? null : next;
    if ((cartLine.method ?? null) !== method) co.setMethod(cartLine.id, method);
  };
  const rails: StepRails = {
    credits: credits?.state.kind === "ready" ? { credits: priceCredits } : null,
    mana: manaPays && mana?.quote?.method === "mana" ? { manaWei: mana.quote.amount } : null,
    combined: mixed,
  };
  // Buy waits until the pick is the checkout's (and priced), or, for the split, until there is a wallet to send it.
  const splitHeld = rail === "combined" ? splitReason(combined.blocked, co.wallet.accountMismatch, co.blocks) : null;
  const stepReady = rail === "combined" ? !splitHeld && checked && !!mixed : rail !== null && selected === rail && (co.canConfirm || reviewRepriced);
  const stepHeld: Blocker | null = rail === "combined" ? splitHeld : held;
  const buyWith = (chosen: StepRail) => {
    if (reviewRepriced) {
      co.acceptChanges();
      return;
    }
    if (chosen === "combined") void combined.start();
    else if (chosen === selected) void co.confirm();
  };

  if (run.kind === "approve") {
    return <AuthorizeStep auth={run.auth} requiredWei={combined.agreed?.manaWei} grant={combined.grant} onAuthorized={combined.approved} onCancel={combined.declined} portal={false} />;
  }

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
    <DialogShell dialog={dialog} label={`Buy ${display.name || TEXT.itemFallback}`} testId="buy-modal" busy={busy} onClose={close} tall={phase === "processing" || phase === "loading"} phase={phase} method={methodMode}>
        {methodMode ? (
          <PaymentMethodStep
            display={display}
            priceCredits={priceCredits}
            rails={rails}
            priceManaWei={priceManaWei}
            balanceCredits={co.balanceError ? null : (co.balance?.credits ?? 0)}
            manaBalanceWei={co.manaBalance === null ? ZERO_MANA : manaWei(co.manaBalance)}
            onPick={pick}
            initial={rail}
            ready={stepReady}
            onBuy={buyWith}
            onClose={close}
            notice={
              priceChanged || stepHeld ? (
                <span className={styles.notes}>
                  {priceChanged ? <span data-testid="price-changed">{TEXT.priceChanged}</span> : null}
                  {stepHeld ? (
                    <span data-testid="buy-modal-held">
                      <HeldText block={stepHeld} wallet={co.wallet} web3={co.session.kind === "web3"} />
                    </span>
                  ) : null}
                </span>
              ) : null
            }
          />
        ) : (
          <DialogHead title={title} balance={balance} busy={busy} onClose={close} testId="buy-modal" />
        )}

        {phase === "loading" ? <LoadingBody /> : null}

        {phase === "error" ? (
          <ErrorBody
            message={run.kind === "error" ? run.message : outage ? TEXT.outage : gone ? TEXT.gone : result?.status === "failed" ? result.failure.message : null}
            onClose={close}
            onRetry={() => {
              // The split's failure goes back to the method step; the checkout's own retries in its place.
              if (run.kind === "error") combined.reset();
              else if (outage || gone) co.revalidate();
              else void co.discardCheckout();
            }}
          />
        ) : null}

        {phase === "nofunds" ? (
          <NoFundsBody
            testId="buy-modal"
            units={1}
            items={<DialogAsset display={display} priceCredits={priceCredits} testId="buy-modal-asset" flapTestId="buy-modal-flap" />}
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

        {phase === "ready" && !methodMode ? (
          <DialogBody>
            {priceChanged ? (
              <DialogWarning status testId="price-changed">
                {TEXT.priceChanged}
              </DialogWarning>
            ) : null}
            <DialogAsset display={display} priceCredits={priceCredits} testId="buy-modal-asset" flapTestId="buy-modal-flap" />
            {held ? (
              <DialogWarning status testId="buy-modal-held">
                <HeldText block={held} wallet={co.wallet} web3={co.session.kind === "web3"} />
              </DialogWarning>
            ) : null}
            <DialogCtas>
              <DialogButton variant="gradient" full onClick={pay} disabled={!co.canConfirm && !reviewRepriced} data-testid="buy-modal-buy">
                {TEXT.buy}
              </DialogButton>
            </DialogCtas>
          </DialogBody>
        ) : null}

        {phase === "processing" ? (
          <ProcessingBody text={resume ? TEXT.completingResume : TEXT.completing} />
        ) : null}

        {phase === "pending" ? (
          <PendingBody text={TEXT.stillConfirming} testId="buy-modal-pending">
            <DialogCtas>
              <DialogButton variant="outline" onClick={close}>
                {TEXT.close}
              </DialogButton>
              {/* A split is checked by its own receipt (none to read before its purchase went out); the checkout's by the checkout. */}
              <DialogButton
                variant="ruby"
                onClick={() => void (splitSent ? combined.checkAgain() : co.resume())}
                disabled={splitSent ? combined.checking || !combined.checkable : co.running}
              >
                {TEXT.checkAgain}
              </DialogButton>
            </DialogCtas>
          </PendingBody>
        ) : null}

        {phase === "complete" ? (
          <DialogBody>
            <Confetti className={styles.confetti} />
            <div className={styles.complete} data-testid="buy-modal-complete">
              <svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true">
                <circle cx="32" cy="32" r="32" className={styles.completeMark} />
                <path d="M20 33l8 8 16-18" fill="none" stroke="#fff" strokeWidth="5" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
              <p className={styles.completeText}>
                <b>{TEXT.successTitle}</b> {TEXT.successBody}
              </p>
            </div>
            <DialogCtas stack>
              <DialogButton variant="outline" onClick={() => navigate(myItemsRouteFor([display.category]))}>
                {TEXT.myItems}
              </DialogButton>
              {isIapMode() ? (
                <DialogLinkButton variant="ruby" href={backpackDeepLink(purchased ?? [{ urn: display.urn }])} onClick={onClose}>
                  {TEXT.goToBackpack}
                </DialogLinkButton>
              ) : (
                <DialogLinkButton variant="ruby" href={jumpUrl()} target="_blank" rel="noreferrer" onClick={onClose}>
                  {TEXT.tryInWorld}
                  <JumpInIcon />
                </DialogLinkButton>
              )}
            </DialogCtas>
          </DialogBody>
        ) : null}
    </DialogShell>
  );
}

/** Why Buy waits, and the way back when it is the wallet: sign in with the wallet's account, or connect it again. */
function HeldText({ block, wallet, web3 }: { block: Blocker; wallet: ReturnType<typeof useCheckout>["wallet"]; web3: boolean }) {
  const how = HELD[block.kind] ?? {};
  const host = wallet.host;
  const action =
    how.action === "signInWithWallet" && host?.onSwitchAccount
      ? { label: TEXT.switchAccount, run: host.onSwitchAccount }
      : how.action === "connectWallet" && web3 && host?.onConnectWallet
        ? { label: TEXT.connectWallet, run: host.onConnectWallet }
        : null;
  return (
    <>
      {how.text ?? block.text}
      {action ? (
        <>
          {" "}
          <DialogTextButton onClick={action.run}>{action.label}</DialogTextButton>
        </>
      ) : null}
    </>
  );
}
