// from: shop@2d3bf62:app/src/pages/Cart.tsx (+ Cart.styles.ts, components/EmptyState, the Top band, Panel/PanelHead
//       "Cart: N Items" + Fitting Room, the Purchase Summary card, the "Goes with your cart" / "You might also
//       like" rails); supersedes v2 ui3/src/marketplace/pages/MkCheckout.tsx and the cart/checkout pages of
//       sites (marketplace.cart.tsx, marketplace.checkout.tsx)
import { useMemo, useState } from "react";
import { Link, useLocation, useNavigate } from "react-router";

import Button from "../../atoms/Button";
import Icon from "../discovery/components/Icon";
import Rail from "../discovery/components/Rail";
import SuggestedForYouRow, { useSuggestedForYou } from "../discovery/components/SuggestedForYouRow";
import { fetchCatalogByIds, fetchListings } from "../discovery/api";
import type { CatalogItem } from "../discovery/lib/catalogItem";
import { suggestedHiddenReason } from "../discovery/lib/suggestionEvents";
import { useShopQuery } from "../discovery/query";
import FittingRoom from "../detail/components/FittingRoom";
import TopUpPanel from "../credits/components/TopUpPanel";
import { saveReceipt, type SuccessNavState } from "../history/receipt";
import type { GuidanceRevalidation } from "../wallet/ManaGuidance";
import AuthorizeStep from "./components/AuthorizeStep";
import CartLineRow from "./components/CartLineRow";
import CartSummary from "./components/CartSummary";
import CheckoutGroups from "./components/CheckoutGroups";
import FlowBanner from "./components/FlowBanner";
import { EnsureQueryClient, useShopServices } from "./services";
import { useCheckout, type CheckoutView } from "./useCheckout";
import cartEmptyIllustration from "./assets/cart-empty.svg?url";
import "./cartpage.css";

export type CartNavState = { resumeCheckout?: boolean; creditsAdded?: number };

function guidanceRevalidation(co: CheckoutView): GuidanceRevalidation | null {
  switch (co.review.kind) {
    case "checking":
      return { status: "checking", onRetry: co.revalidate };
    case "current":
      return { status: "current", onRetry: co.revalidate };
    case "changed":
      return { status: "changed", onRetry: co.revalidate };
    case "outage":
      return { status: "failed", onRetry: co.revalidate };
    default:
      return null;
  }
}

const CART_RAIL_SIZE = 12;

function lineItemKey(line: { asset: { contract: string; token: { kind: string; itemId?: string } } }): string | null {
  return line.asset.token.kind === "item" && line.asset.token.itemId ? `${line.asset.contract.toLowerCase()}-${line.asset.token.itemId}` : null;
}

function CartUpsell({ exclude }: { exclude: string[] }) {
  const personal = useSuggestedForYou(CART_RAIL_SIZE, { exclude });
  const personalHidden = suggestedHiddenReason({
    enabled: personal.enabled,
    hasSignal: personal.hasSignal,
    isLoading: personal.isLoading,
    isError: personal.isError,
    personalized: personal.result?.personalized,
    rowCount: personal.result?.data?.length ?? 0,
  });
  const generic = useShopQuery(personalHidden !== null ? ["upsell-listings"] : null, (signal) => fetchListings({ first: 40 }, { signal }), { staleMs: 60_000 });
  const skip = new Set(exclude);
  const upsell = (generic.data?.items ?? []).filter((i) => !(i.itemId && skip.has(`${i.contractAddress.toLowerCase()}-${i.itemId}`))).slice(0, CART_RAIL_SIZE);
  if (personalHidden === null) {
    return (
      <div className="shop-cart__upsell" data-testid="cart-personal-upsell">
        <SuggestedForYouRow exclude={exclude} title="Goes with your cart" first={CART_RAIL_SIZE} />
      </div>
    );
  }
  if (upsell.length === 0) return null;
  return (
    <div className="shop-cart__upsell">
      <Rail title="You might also like" items={upsell} source="related" testId="cart-upsell" />
    </div>
  );
}

function CartScreen() {
  const navigate = useNavigate();
  const location = useLocation();
  const services = useShopServices();
  const nav = (location.state ?? {}) as CartNavState;
  const [topUpOpen, setTopUpOpen] = useState(false);
  const [fitting, setFitting] = useState<CatalogItem[] | null>(null);
  const [fittingBusy, setFittingBusy] = useState(false);

  const co = useCheckout({
    autoConfirm: !!nav.resumeCheckout,
    onComplete: (purchased, results, groups) => {
      const txs = groups.flatMap((g) => {
        const r = results[g.keyId];
        return r?.status === "confirmed" && r.txHash ? [{ hash: r.txHash, chainId: g.key.chainId, method: g.key.method }] : [];
      });
      const state: SuccessNavState = {
        items: purchased.map((p) => ({
          name: p.name,
          thumbnail: p.thumbnail,
          href: p.href,
          quantity: p.quantity,
          method: p.method,
          creator: p.creator,
          category: p.category,
          urn: p.urn,
          contractAddress: p.contractAddress,
          itemId: p.itemId,
          ...(p.method === "credits" && p.credits !== undefined ? { credits: p.credits } : {}),
        })),
        txs,
        settled: true,
        ...(nav.creditsAdded ? { creditsAdded: nav.creditsAdded } : {}),
      };
      saveReceipt(state);
      navigate("/shop/success", { state, replace: true });
    },
  });

  const executorTopUp = Object.values(co.results).some((r) => r.status === "failed" && !!r.topUp);
  const host = co.wallet.host;
  const guidanceProps = useMemo(
    () => ({
      returnTo: "/shop/cart",
      wallet: { account: co.wallet.walletAccount, chainId: co.wallet.chainId, pendingChainId: co.wallet.pendingChainId, manaBalance: null },
      onSignIn: host?.onSignIn,
      onConnectWallet: host?.onConnectWallet,
      onSwitchAccount: host?.onSwitchAccount,
      onSwitchNetwork: (chainId: number) => void co.wallet.switchTo(chainId),
      setupUrl: services.walletSetupUrl,
      revalidation: guidanceRevalidation(co),
    }),
    [co, host, services.walletSetupUrl],
  );

  const inFlight = !!co.checkout && !!co.summary && !co.summary.complete;
  const broadcasting = !!co.checkout?.groups.some((g) => g.status === "submitted" || g.status === "pending");
  const lineUnitsOf = (l: (typeof co.lines)[number]) => (l.asset.token.kind === "item" ? l.quantity : 1);
  const units = co.lines.reduce((n, l) => n + lineUnitsOf(l), 0);
  const isGone = (id: string) => (co.changesByLine[id] ?? []).some((c) => c.kind === "unavailable" || (c.kind === "insufficientStock" && c.available === 0));
  const buyableUnits = co.lines.reduce((n, l) => n + (isGone(l.id) ? 0 : lineUnitsOf(l)), 0);
  const allUnavailable = co.lines.length > 0 && buyableUnits === 0;
  const signedOut = co.session.kind === "signedOut";
  const reviewing = co.review.kind === "changed" && co.reviewChanges.length > 0;
  const exclude = useMemo(() => co.lines.map(lineItemKey).filter((k): k is string => !!k), [co.lines]);
  const wearableKeys = co.lines.filter((l) => l.display.category === "wearable").map(lineItemKey).filter((k): k is string => !!k);

  async function openFitting() {
    if (wearableKeys.length === 0) return;
    setFittingBusy(true);
    try {
      const items = await fetchCatalogByIds(wearableKeys);
      if (items.length > 0) setFitting(items);
    } catch {
    } finally {
      setFittingBusy(false);
    }
  }

  if (co.lines.length === 0 && !inFlight) {
    return (
      <main className="shop-cart" aria-labelledby="shop-cart-title">
        <div className="shop-cart__band">
          <div className="shop-cart__inner">
            <h1 id="shop-cart-title" className="u-sr-only">
              Cart
            </h1>
            <div className="shop-cart__empty" data-testid="cart-empty">
              <img className="shop-cart__empty-art" src={cartEmptyIllustration} alt="" aria-hidden />
              <div className="shop-cart__empty-text">
                <p className="shop-cart__empty-title">Nothing here... yet!</p>
                <p className="shop-cart__empty-body">Explore the Shop and discover your next favorite item.</p>
              </div>
              <Link to="/shop/items" className="shop-cart__empty-cta">
                Start shopping
              </Link>
            </div>
          </div>
        </div>
      </main>
    );
  }

  const ctaLabel = co.running ? "Working…" : "Checkout";

  return (
    <main className="shop-cart" aria-labelledby="shop-cart-title">
      <div className="shop-cart__band">
        <div className="shop-cart__inner">
          <div className="shop-cart__notices">
            {co.wallet.accountMismatch ? (
              <FlowBanner
                tone="error"
                title="Your wallet switched accounts"
                action={host?.onSwitchAccount ? <Button size="sm" variant="secondary" onClick={host.onSwitchAccount}>Sign in with this wallet</Button> : undefined}
              >
                You're signed in as {co.account}, but your wallet is on {co.wallet.walletAccount}. Items go to the signed-in account, so sign in again with the wallet's account or switch the wallet back.
              </FlowBanner>
            ) : null}
            {reviewing ? (
              <FlowBanner tone="warning" title="Something changed since you added it" action={<Button size="sm" variant="secondary" onClick={co.acceptChanges}>Accept the changes</Button>}>
                Review the items marked below. New prices are only charged after you accept them.
              </FlowBanner>
            ) : null}
            {co.review.kind === "outage" ? (
              <FlowBanner tone="outage" title="Prices can't be checked right now" action={co.review.reason === "failed" ? <Button size="sm" variant="secondary" onClick={co.revalidate}>Try again</Button> : undefined}>
                Your cart and each item's way to pay are kept. Nothing can be bought until prices and availability are confirmed{co.review.reason === "no-quote-route" ? " by the Shop" : ""}.
              </FlowBanner>
            ) : null}
            {co.review.kind !== "outage" && co.quoteOutage.length > 0 ? (
              <FlowBanner tone="outage" title={co.quoteOutage.includes("credits") ? "The Credits price is unavailable right now" : "The MANA price is unavailable right now"} action={<Button size="sm" variant="secondary" onClick={co.revalidate}>Try again</Button>}>
                Each item keeps its way to pay. Items priced right now can't be bought until the price is back; nothing moves to another way to pay.
              </FlowBanner>
            ) : null}
            {inFlight && !co.running ? (
              <FlowBanner
                tone="warning"
                title={broadcasting ? "A purchase from this cart is still being confirmed" : "A purchase from this cart didn't finish"}
                action={
                  <span className="shop-cart__actions">
                    <Button size="sm" onClick={() => void co.resume()} disabled={!co.account || (!broadcasting && !co.canResume)} data-testid="resume-checkout">
                      {broadcasting ? "Check again" : "Resume"}
                    </Button>
                    {!broadcasting ? (
                      <Button size="sm" variant="ghost" onClick={() => void co.discardCheckout()}>
                        Start over
                      </Button>
                    ) : null}
                  </span>
                }
              >
                {broadcasting
                  ? "Payments that were sent are only watched, never sent twice. Those that didn't go out can be retried."
                  : "Only the payments that didn't complete will run again."}
              </FlowBanner>
            ) : null}
            {co.settlementIssues?.unsupported.length ? (
              <FlowBanner tone="info" title="Some items finish on their own page">
                NAME registrations, rentals and older listings are bought from the item's page. The rest of the cart can be bought here.
              </FlowBanner>
            ) : null}
          </div>

          <div className="shop-cart__body">
            <div className="shop-cart__left">
              <section className="shop-cart__panel" aria-labelledby="shop-cart-title">
                <div className="shop-cart__panelhead">
                  <button type="button" className="shop-cart__panelback" onClick={() => navigate(-1)} aria-label="Go back">
                    <Icon name="arrow-left" size={20} />
                  </button>
                  <h1 id="shop-cart-title" className="shop-cart__title">
                    Cart: {units} {units === 1 ? "Item" : "Items"}
                  </h1>
                  {wearableKeys.length > 0 ? (
                    <button type="button" className="shop-cart__fitting" onClick={() => void openFitting()} disabled={co.running || fittingBusy}>
                      <Icon name="fitting-room" size={20} />
                      Fitting Room
                    </button>
                  ) : null}
                </div>

                <div className="shop-cart__lines" aria-label="Items in your cart">
                  {co.lines.map((line) => (
                    <CartLineRow
                      key={line.id}
                      line={line}
                      offer={co.offers[line.id] ?? null}
                      view={co.views[line.id] ?? null}
                      changes={co.changesByLine[line.id] ?? []}
                      refusal={co.refusedByLine[line.id]}
                      disabled={co.running}
                      onMethod={(m) => co.setMethod(line.id, m)}
                      onQuantity={(q) => co.setQuantity(line.id, q)}
                      onRemove={() => co.remove(line.id)}
                      onRetry={co.revalidate}
                      guidance={guidanceProps}
                    />
                  ))}
                </div>

                <div className="shop-cart__utils">
                  <button type="button" className="shop-cart__clear" onClick={co.clear} disabled={co.running}>
                    Clear cart
                  </button>
                </div>
              </section>
            </div>

            <aside className="shop-cart__side" aria-label="Purchase summary">
              <h2 className="shop-cart__subtitle">Purchase Summary</h2>
              <div className="shop-cart__sidebody">
                <CartSummary
                  partition={co.partition}
                  balance={co.balance}
                  balanceError={co.balanceError}
                  signedIn={!signedOut}
                  units={buyableUnits}
                  onTopUp={() => setTopUpOpen(true)}
                />
                <CheckoutGroups
                  groups={co.partition?.groups ?? []}
                  lines={co.lines}
                  progress={co.checkout?.groups ?? []}
                  results={co.results}
                  walletNeededKeys={co.walletNeededKeys}
                />
                {co.topUp && (topUpOpen || executorTopUp) ? (
                  <TopUpPanel
                    shortCents={co.topUp.shortCents}
                    onBeforeLeave={co.stashForTopUp}
                    onCheckout={(s) => (s.url ? services.openExternal(s.url) : navigate(`/shop/credits?order=${encodeURIComponent(s.orderId)}`))}
                    onCancel={() => {
                      setTopUpOpen(false);
                      co.dismissTopUp();
                    }}
                  />
                ) : null}
                <button
                  type="button"
                  className="shop-cart__confirm"
                  onClick={() => void co.confirm()}
                  disabled={co.running || allUnavailable || (!signedOut && !co.canConfirm)}
                  data-testid="confirm-checkout"
                >
                  {ctaLabel}
                </button>
                {allUnavailable ? <p className="shop-cart__msg">None of these items are available anymore. Remove them to keep shopping.</p> : null}
                {signedOut ? <p className="shop-cart__msg">You'll be asked to sign in first.</p> : null}
                {co.blockers.length > 0 && !signedOut ? (
                  <ul className="shop-cart__blockers" aria-label="Why you can't confirm yet">
                    {co.blockers.map((b) => (
                      <li key={b} className="shop-cart__msg">
                        {b}
                      </li>
                    ))}
                  </ul>
                ) : null}
              </div>
            </aside>
          </div>
        </div>
      </div>

      <CartUpsell exclude={exclude} />

      {fitting ? <FittingRoom items={fitting} onClose={() => setFitting(null)} /> : null}

      {co.approval ? (
        <AuthorizeStep
          auth={co.approval.request.auth}
          step={{ index: co.approval.request.index, total: co.approval.request.total }}
          requiredWei={co.approval.request.requiredWei}
          grant={co.approval.grant}
          onAuthorized={() => co.approval?.resolve(true)}
          onCancel={() => co.approval?.resolve(false)}
        />
      ) : null}
    </main>
  );
}

export default function CartPage() {
  return (
    <EnsureQueryClient>
      <CartScreen />
    </EnsureQueryClient>
  );
}
