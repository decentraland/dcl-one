// from: shop@2d3bf62:app/src/pages/Cart.tsx (the summary panel: TotalLine "Total: N items" + TotalValue), app/src/components/PaymentCtas/PaymentCtas.tsx
import type { CartPartition } from "../../cart/partition";
import { chainLabel } from "../../model/chain";
import { creditsFloor, formatUsd } from "../../model/money";
import type { UsdBalance } from "../../credits/credits";
import { CreditsAmount, ManaAmount } from "../../credits/components/Amount";
import HeldCredits from "../../credits/components/HeldCredits";
import "./cartsummary.css";

export type CartSummaryProps = {
  partition: CartPartition | null;
  balance: UsdBalance | undefined;
  balanceError?: boolean;
  signedIn: boolean;
  units: number;
  onTopUp?(): void;
};

export default function CartSummary({ partition, balance, balanceError = false, signedIn, units, onTopUp }: CartSummaryProps) {
  const credits = partition?.totals.credits;
  const manaByChain = new Map<number, bigint>();
  for (const g of partition?.groups ?? []) {
    if (g.total.method === "mana") manaByChain.set(g.key.chainId, (manaByChain.get(g.key.chainId) ?? 0n) + g.total.amount);
  }
  const short = partition?.creditsShort ?? null;

  return (
    <dl className="shop-summary" aria-label="Totals">
      <div className="shop-summary__total" data-testid="cart-total">
        <dt>Total: {units} {units === 1 ? "item" : "items"}</dt>
        <dd className="shop-summary__values">
          {credits && credits.charged > 0 ? (
            <span className="shop-summary__value" data-testid="total-credits">
              <CreditsAmount credits={creditsFloor(credits.charged)} size={24} />
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
      {signedIn && credits && credits.charged > 0 ? (
        <div className="shop-summary__row shop-summary__row--balance">
          <dt>Your Credits</dt>
          <dd>
            {balanceError ? <span aria-label="Balance unavailable">—</span> : <CreditsAmount credits={balance?.credits ?? 0} />}
            <HeldCredits held={balance?.held} />
          </dd>
        </div>
      ) : null}
      {short ? (
        <div className="shop-summary__short" role="status">
          <span>You need {Math.ceil(short / 10)} more Credits.</span>
          {onTopUp ? (
            <button type="button" className="shop-summary__topup" onClick={onTopUp} data-testid="open-topup">
              Top up
            </button>
          ) : null}
        </div>
      ) : null}
    </dl>
  );
}
