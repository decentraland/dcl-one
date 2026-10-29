// from: shop@94584a5c:app/src/components/CreditRate/CreditRate.tsx
// "1 Credit = USD 0.10": the peg, stated wherever credits are bought or priced. Both numbers come from
// the currency helpers, so the line can never drift from the conversion.

import { CURRENCY, creditsToUsd } from "../../checkout/lib/currency";
import CreditMark from "./CreditMark";
import "./creditrate.css";

export default function CreditRate({ align }: { align?: "center" }) {
  const usd = creditsToUsd(1).toFixed(2);
  return (
    <p className="shop-credit-rate" data-align={align} aria-label={`1 ${CURRENCY.nameSingular} equals ${usd} US dollars`}>
      <CreditMark size={14} />
      <span>
        <b>1</b> {CURRENCY.nameSingular}
      </span>
      <span aria-hidden="true">=</span>
      <span>
        USD <b>{usd}</b>
      </span>
    </p>
  );
}
