// from: shop@2d3bf62:app/src/pages/ItemDetail.tsx (the price block and CTAs), app/src/components/PaymentCtas/PaymentCtas.tsx,
//       app/src/components/PaymentMethodStep/PaymentMethodStep.tsx (the rails), app/src/components/ManaPricingBanner/ManaPricingBanner.tsx

import { useEffect, useId, useState } from "react";
import { Link } from "react-router";

import Button from "../../../atoms/Button";
import Icon from "../../discovery/components/Icon";
import { pdpText } from "../copy";
import { chainLabel } from "../../model/chain";
import { creditsCeil, formatMana, type ManaWei, type UsdCents } from "../../model/money";
import type { PurchaseOffer } from "../../model/purchase";
import type { Eligibility, PaymentMethod, Reason, Session } from "../../model/settlement";
import { optionStateText, outageText, reasonText } from "../../payment/copy";
import { presentPayment, type MixedPayment, type OptionState, type PaymentOptionView } from "../../payment/present";
import { SHOP_BASE } from "../../routes";
import ManaGuidance, { type GuidanceRevalidation } from "../../wallet/ManaGuidance";
import { CreditsAmount, ManaAmount } from "../../discovery/components/PriceTag";
import { t } from "../../discovery/intl";
import type { OffersStatus } from "../offers";
import "./paymentpanel.css";

export type ListingPrice = {
  eligibility: Eligibility;
  credits: number | null;
  mana: ManaWei | null;
  chainId: number;
};

export type WalletView = {
  account: string | null;
  walletAccount: string | null;
  chainId: number | null;
  pendingChainId: number | null;
  accountMismatch: boolean;
  onSignIn?(): void;
  onConnectWallet?(): void;
  onSwitchAccount?(): void;
  onSwitchNetwork?(chainId: number): void;
};

export type PaymentPanelProps = {
  offer: PurchaseOffer | null;
  status: OffersStatus;
  listing: ListingPrice | null;
  session: Session;
  wallet: WalletView;
  item: { name: string; href: string };
  creditsBalance?: UsdCents | null;
  manaBalance?: ManaWei | null;
  inCart?: boolean;
  atStockCap?: boolean;
  stock?: { supply: number; max: number } | null;
  setupUrl?: string;
  revalidation?: GuidanceRevalidation | null;
  onRetry(): void;
  onBuy(method: PaymentMethod, offer: PurchaseOffer, mixed?: MixedPayment): void;
  onAddToCart(method: PaymentMethod | null): void;
};

type OptionRow = { method: PaymentMethod; view: PaymentOptionView | null; state: OptionState };

function manaChainOf(offer: PurchaseOffer | null, listing: ListingPrice | null): number {
  return offer?.settlement.find((s) => s.method === "mana")?.key.chainId ?? offer?.asset.chainId ?? listing?.chainId ?? 137;
}

function OptionPrice({ row, listing, network, testId = `price-${row.method}` }: { row: OptionRow; listing: ListingPrice | null; network: "polygon" | "ethereum"; testId?: string }) {
  const quote = row.view?.quote;
  if (quote?.method === "credits") return <CreditsAmount credits={creditsCeil(quote.quoted)} testId={testId} />;
  if (quote?.method === "mana") return <ManaAmount wei={quote.amount} network={network} testId={testId} />;
  if (row.method === "credits" && listing?.credits) return <CreditsAmount credits={listing.credits} testId={testId} />;
  if (row.method === "mana" && listing?.mana) return <ManaAmount wei={listing.mana} network={network} testId={testId} />;
  return (
    <span className="shop-pay__noprice" data-testid={testId}>
      {"—"}
    </span>
  );
}

export default function PaymentPanel(props: PaymentPanelProps) {
  const { offer, status, listing, session, wallet, item } = props;
  const groupName = useId();
  const presentation = offer ? presentPayment(offer, session, { creditsBalance: props.creditsBalance, manaBalance: props.manaBalance }) : null;
  const quoteFailed = !offer && (status === "routeMissing" || status === "failed");
  const deferToSignIn = quoteFailed && session.kind === "signedOut" && !!wallet.onSignIn;
  const outage = quoteFailed && !deferToSignIn;
  const eligibility = presentation ? null : (listing?.eligibility ?? null);

  const rows: OptionRow[] = presentation
    ? presentation.options.map((view) => ({ method: view.method, view, state: view.state }))
    : (eligibility?.allowed ?? []).map((method) => ({ method, view: null, state: { kind: "unavailable" as const, missing: null } }));
  const primary: PaymentMethod | null = presentation?.primary ?? eligibility?.default ?? null;
  const manaOnly = presentation ? presentation.manaOnly : !!eligibility && !eligibility.allowed.includes("credits");
  const reasons: Reason[] = presentation?.reasons ?? eligibility?.reasons ?? [];
  const guidance = presentation?.guidance ?? eligibility?.guidance ?? null;
  const chainId = manaChainOf(offer, listing);
  const network = chainId === 1 || chainId === 11_155_111 ? "ethereum" : "polygon";

  const [selected, setSelected] = useState<PaymentMethod | null>(primary);
  const [useMixed, setUseMixed] = useState(false);
  useEffect(() => {
    setSelected((current) => (current && rows.some((r) => r.method === current) ? current : primary));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [primary, rows.map((r) => r.method).join(",")]);

  if (status === "loading" && !offer) {
    return (
      <div className="shop-pay" aria-busy="true" data-testid="payment-loading">
        <div className="shop-pay__skeleton" />
        <div className="shop-pay__skeleton shop-pay__skeleton--cta" />
        <div className="shop-pay__skeleton shop-pay__skeleton--cta" />
      </div>
    );
  }
  if (rows.length === 0) {
    return (
      <div className="shop-pay" data-testid="payment-none">
        <p className="shop-pay__none">{t("itemDetail.notForSale")}</p>
      </div>
    );
  }

  const current = rows.find((r) => r.method === selected) ?? rows[0]!;
  const mixed = current.method === "mana" ? (rows.find((r) => r.method === "mana")?.view?.mixed ?? null) : null;
  const splitting = !!mixed && useMixed;
  const manaQuote = presentation?.options.find((o) => o.method === "mana")?.quote;
  const requiredMana = manaQuote?.method === "mana" ? manaQuote.amount : (listing?.mana ?? null);
  const ready = !!offer && current.state.kind === "ready" && !(current.method === "mana" && wallet.accountMismatch);
  const buyLabel = current.method === "credits" ? t("itemDetail.buyWithCredits") : splitting ? t("itemDetail.buyWithMixed") : t("itemDetail.buyWithMana");

  const stock = props.stock ?? null;
  return (
    <div className="shop-pay" data-testid="payment-panel" data-mana-only={manaOnly || undefined}>
      <div className="shop-pay__block">
        <div className="shop-pay__row">
          <div className="shop-pay__pricecol" data-method={current.method}>
            <span className="shop-pay__caption">{t("itemDetail.price")}</span>
            <div className="shop-pay__price" data-testid="item-price">
              <OptionPrice row={current} listing={listing} network={network} testId={rows.length > 1 ? "price-current" : undefined} />
            </div>
          </div>
          {stock ? (
            <div className="shop-pay__stockcol" data-testid="detail-stock">
              <span className="shop-pay__caption">{t("itemDetail.stock")}</span>{" "}
              <span className="shop-pay__stock">
                {stock.supply.toLocaleString("en-US")}/{stock.max.toLocaleString("en-US")}
              </span>
            </div>
          ) : null}
        </div>
        {rows.length > 1 ? (
          <fieldset className="shop-pay__options">
            <legend>{t("itemDetail.payWith")}</legend>
            {rows.map((row) => (
              <label key={row.method} className="shop-pay__option" data-method={row.method} data-selected={row.method === current.method || undefined}>
                <input type="radio" name={groupName} value={row.method} checked={row.method === current.method} onChange={() => setSelected(row.method)} data-testid={`pay-${row.method}`} />
                <span className="shop-pay__label">{row.method === "credits" ? t("itemDetail.creditsOption") : t("itemDetail.manaOption")}</span>
                <OptionPrice row={row} listing={listing} network={network} />
              </label>
            ))}
          </fieldset>
        ) : null}
      </div>

      {current.method === "mana" && mixed ? (
        <label className="shop-pay__mixed" data-testid="mixed-payment">
          <input type="checkbox" checked={useMixed} onChange={(e) => setUseMixed(e.target.checked)} data-testid="pay-mixed" />
          {t("itemDetail.mixedOption", { credits: t("assetCard.credits", { count: mixed.credits }), mana: formatMana(mixed.manaWei, { group: true }) })}
        </label>
      ) : null}

      {current.method === "mana" ? (
        <p className="shop-pay__network" data-testid="mana-network">
          {t("itemDetail.manaNetwork", { network: chainLabel(chainId) })}
        </p>
      ) : null}

      {manaOnly && reasons.length > 0 ? (
        <ul className="shop-pay__reasons" data-testid="mana-only-reasons">
          {reasons.map((r) => (
            <li key={r}>{reasonText(r)}</li>
          ))}
        </ul>
      ) : null}

      {outage ? (
        <div className="shop-pay__outage" role="status" data-testid="payment-outage">
          <p>{status === "routeMissing" ? t("itemDetail.quoteRouteMissing") : t("itemDetail.quoteFailed")}</p>
          <Button variant="secondary" size="sm" onClick={props.onRetry}>
            {t("itemDetail.retryPrice")}
          </Button>
        </div>
      ) : current.state.kind === "unavailable" && !deferToSignIn ? (
        <div className="shop-pay__outage" role="status" data-testid="payment-outage">
          <p>{outageText(current.method, current.state.missing, current.state.withheld)}</p>
          <Button variant="secondary" size="sm" onClick={props.onRetry}>
            {t("itemDetail.retryPrice")}
          </Button>
        </div>
      ) : current.state.kind !== "ready" && current.state.kind !== "walletRequired" && !deferToSignIn ? (
        <p className="shop-pay__state" role="status" data-testid={`state-${current.state.kind}`}>
          {optionStateText(current.method, current.state)}
          {current.state.kind === "topUpRequired" ? ` ${t("itemDetail.topUpShort", { credits: t("assetCard.credits", { count: creditsCeil(current.state.short) }) })}` : ""}
        </p>
      ) : null}

      {wallet.accountMismatch && current.method === "mana" ? (
        <div className="shop-pay__mismatch" role="alert" data-testid="account-mismatch">
          <p>{t("itemDetail.accountMismatch", { wallet: wallet.walletAccount ?? "", account: wallet.account ?? "" })}</p>
          {wallet.onSwitchAccount ? (
            <Button variant="secondary" size="sm" onClick={wallet.onSwitchAccount}>
              {t("itemDetail.switchAccount")}
            </Button>
          ) : null}
        </div>
      ) : null}

      {guidance && current.method === "mana" ? (
        <ManaGuidance
          guidance={guidance}
          required={requiredMana}
          reasons={reasons}
          item={item}
          returnTo={item.href}
          wallet={wallet.walletAccount ? { account: wallet.walletAccount, chainId: wallet.chainId, manaBalance: props.manaBalance ?? null, pendingChainId: wallet.pendingChainId } : undefined}
          onSignIn={wallet.onSignIn}
          onConnectWallet={wallet.onConnectWallet}
          onSwitchAccount={wallet.onSwitchAccount}
          onSwitchNetwork={wallet.onSwitchNetwork}
          setupUrl={props.setupUrl}
          revalidation={quoteFailed ? null : props.revalidation}
        />
      ) : null}

      <div className="shop-pay__ctas" data-dual>
        {(current.state.kind === "signInRequired" || deferToSignIn) && wallet.onSignIn ? (
          <button type="button" className="shop-pay__buy" onClick={wallet.onSignIn} data-testid="cta-sign-in">
            <span>{pdpText("assetCard.buyNow")}</span>
            <span className="shop-pay__ctaprice" aria-hidden>
              <OptionPrice row={current} listing={listing} network={network} testId="" />
            </span>
          </button>
        ) : current.state.kind === "topUpRequired" ? (
          <Link className="shop-pay__buy" to={`${SHOP_BASE}/credits?returnTo=${encodeURIComponent(item.href)}`} data-testid="cta-top-up">
            {t("itemDetail.topUp")}
          </Link>
        ) : current.state.kind === "walletRequired" && !guidance && wallet.onConnectWallet ? (
          <button type="button" className="shop-pay__buy" onClick={wallet.onConnectWallet} data-testid="cta-connect">
            {t("itemDetail.connectWallet")}
          </button>
        ) : (
          <button type="button" className="shop-pay__buy" onClick={() => offer && props.onBuy(current.method, offer, splitting ? mixed! : undefined)} disabled={!ready} data-testid="cta-buy">
            <span>{buyLabel}</span>
            <span className="shop-pay__ctaprice" aria-hidden>
              <OptionPrice row={current} listing={listing} network={network} testId="" />
            </span>
          </button>
        )}
        <button
          type="button"
          className="shop-pay__cart"
          onClick={() => props.onAddToCart(current.method === primary ? null : current.method)}
          disabled={props.inCart || props.atStockCap}
          aria-label={props.atStockCap ? t("itemDetail.maxInCart") : props.inCart ? t("itemDetail.inCart") : t("itemDetail.addToCart")}
          data-testid="cta-add-to-cart"
        >
          <Icon name="cart" size={20} />
          <span className="shop-pay__cartlabel">{props.atStockCap ? t("itemDetail.maxInCart") : props.inCart ? t("itemDetail.inCart") : t("itemDetail.addToCart")}</span>
        </button>
      </div>
    </div>
  );
}
