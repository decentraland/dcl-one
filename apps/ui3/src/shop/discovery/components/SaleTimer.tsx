// from: shop@94584a5c:app/src/components/SaleTimer/SaleTimer.tsx, app/src/components/OfferStock/OfferStock.tsx
import { useEffect, useState } from "react";

import { t } from "../intl";
import { countdownParts, formatCountdown, saleTimeLeft } from "../lib/sale";
import Icon from "./Icon";
import "./sale.css";

/** "Ends in 2d 4h": ticks every second, disappears when the window closes. */
export function SaleTimer({ until, testId }: { until?: number; testId?: string }) {
  const [left, setLeft] = useState(() => saleTimeLeft(until));
  useEffect(() => {
    setLeft(saleTimeLeft(until));
    if (until == null) return;
    const timer = setInterval(() => setLeft(saleTimeLeft(until)), 1000);
    return () => clearInterval(timer);
  }, [until]);
  if (!countdownParts(left)) return null;
  return (
    <span className="shop-saletimer" data-testid={testId}>
      <Icon name="clock" size={14} />
      <span>{t("saleTimer.lead")}</span>
      <strong>{formatCountdown(left)}</strong>
    </span>
  );
}

/** A coupon's scale: how many of its uses are claimed. */
export function OfferStock({ claimed, total, testId }: { claimed: number; total: number; testId?: string }) {
  if (!Number.isFinite(claimed) || !Number.isFinite(total) || total <= 0) return null;
  const taken = Math.min(total, Math.max(0, claimed));
  const pct = Math.round((taken / total) * 100);
  const label = t("offerStock.claimed", { claimed: taken, total });
  return (
    <div className="shop-offerstock" data-testid={testId}>
      <div className="shop-offerstock__head">
        <span>{t("offerStock.label")}</span>
        <span>{label}</span>
      </div>
      <div className="shop-offerstock__track" role="progressbar" aria-valuemin={0} aria-valuemax={total} aria-valuenow={taken} aria-label={label}>
        <span style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}
