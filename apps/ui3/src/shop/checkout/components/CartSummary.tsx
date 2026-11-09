// from: shop@3a322cf9:app/src/pages/Cart.tsx (the summary panel: TotalLine "Total: N items" + TotalValue).
// The total alone, as upstream draws it: the balance and a shortfall are not stated here, the rail buttons
// below (PaymentCtas) are the choice, and the credits fallback opens the top-up.
import type { CartPartition } from "@shop/cart/partition";
import { chainLabel } from "@shop/model/chain";
import { creditsFloor, formatUsd } from "@shop/model/money";
import { ManaAmount } from "../../credits/components/Amount";
import { CreditsAmount as ShopCredits } from "../../discovery/components/PriceTag";
import "./cartsummary.css";

export type CartSummaryProps = {
  partition: CartPartition | null;
  units: number;
};

export default function CartSummary({ partition, units }: CartSummaryProps) {
  const credits = partition?.totals.credits;
  const manaByChain = new Map<number, bigint>();
  for (const g of partition?.groups ?? []) {
    if (g.total.method === "mana") manaByChain.set(g.key.chainId, (manaByChain.get(g.key.chainId) ?? 0n) + g.total.amount);
  }

  return (
    <dl className="shop-summary" aria-label="Totals">
      <div className="shop-summary__total">
        <dt>Total: {units} {units === 1 ? "item" : "items"}</dt>
        <dd className="shop-summary__values">
          {credits && credits.charged > 0 ? (
            <span className="shop-summary__value" data-testid="total-credits">
              <ShopCredits credits={creditsFloor(credits.charged)} className="shop-summary__amount" coinClassName="shop-summary__coin" />
              {credits.quoted !== credits.charged ? <span className="shop-summary__sub">priced {formatUsd(credits.quoted)}</span> : null}
            </span>
          ) : null}
          {[...manaByChain].map(([chainId, wei]) => (
            <span key={chainId} className="shop-summary__value" data-testid={`total-mana-${chainId}`}>
              <span className="shop-summary__sub">MANA on {chainLabel(chainId)}</span>
              <ManaAmount wei={wei} chainId={chainId} size={24} />
            </span>
          ))}
          {!(credits && credits.charged > 0) && manaByChain.size === 0 ? (
            <span className="shop-summary__value shop-summary__na" aria-label="Total not priced yet">
              —
            </span>
          ) : null}
        </dd>
      </div>
    </dl>
  );
}
