// from: shop@2d3bf62:app/src/components/Price/Price.tsx, app/src/components/CurrencyIcon/CurrencyIcon.tsx, app/src/lib/currency.ts (formatCredits)

import ManaMark from "../../../atoms/ManaMark";
import { formatMana, type ManaWei } from "../../model/money";
import type { CardPrice } from "../cardPrice";
import { t } from "../intl";
import Icon from "./Icon";
import "./pricetag.css";

const compact = new Intl.NumberFormat("en-US", { notation: "compact", maximumFractionDigits: 2 });
const full = new Intl.NumberFormat("en-US");

export function formatCredits(n: number): string {
  return compact.format(n);
}

export function formatCreditsFull(n: number): string {
  return full.format(n);
}

export function CreditsAmount({ credits, testId, strike = false }: { credits: number; testId?: string; strike?: boolean }) {
  return (
    <span className={`shop-price__credits${strike ? " shop-price__credits--was" : ""}`} data-testid={testId} title={t("assetCard.credits", { count: credits })}>
      {strike ? null : <Icon name="credits" size={15} className="shop-price__mark" />}
      <span aria-hidden>{formatCredits(credits)}</span>
      <span className="shop-sr">{t("assetCard.credits", { count: credits })}</span>
    </span>
  );
}

export function ManaAmount({ wei, network = "polygon", testId }: { wei: ManaWei; network?: "polygon" | "ethereum"; testId?: string }) {
  const amount = formatMana(wei, { group: true });
  return (
    <span className="shop-price__mana" data-testid={testId} title={`${amount} MANA`}>
      <ManaMark size={14} network={network} className="shop-price__mark" />
      <span>{amount}</span>
      <span className="shop-sr"> MANA</span>
    </span>
  );
}

export default function PriceTag({ price, network, compactView = false }: { price: CardPrice; network?: "polygon" | "ethereum"; compactView?: boolean }) {
  switch (price.kind) {
    case "credits":
      return (
        <span className="shop-price" data-kind="credits" data-sale={price.compareAt ? "" : undefined} data-testid="card-price">
          <CreditsAmount credits={price.credits} testId="card-price-now" />
          {price.compareAt ? <CreditsAmount credits={price.compareAt} strike testId="card-price-was" /> : null}
          {price.mana && !compactView ? (
            <span className="shop-price__or" data-testid="card-price-mana">
              {t("assetCard.orMana", { amount: formatMana(price.mana, { group: true }) })}
            </span>
          ) : null}
        </span>
      );
    case "creditsUnavailable":
      return (
        <span className="shop-price" data-kind="credits-unavailable" data-testid="card-price-unavailable">
          <Icon name="warning-fill" size={14} />
          <span>{t("assetCard.creditsUnavailable")}</span>
        </span>
      );
    case "mana":
      return (
        <span className="shop-price" data-kind="mana" data-testid="card-price">
          <ManaAmount wei={price.mana} network={network} />
        </span>
      );
    case "manaAtCheckout":
      return (
        <span className="shop-price" data-kind="mana-at-checkout" data-testid="card-price" title={t("assetCard.manaPriceOnPage")}>
          <ManaMark size={14} network={network} className="shop-price__mark" />
          <span>{t("assetCard.manaOnly")}</span>
        </span>
      );
    case "soldOut":
      return (
        <span className="shop-price" data-kind="sold-out" data-testid="card-sold-out">
          {t("assetCard.soldOut")}
        </span>
      );
    case "notForSale":
      return (
        <span className="shop-price" data-kind="not-for-sale" data-testid="card-nfs">
          {t("assetCard.notForSale")}
        </span>
      );
  }
}
