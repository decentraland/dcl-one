// The two money figures the Shop shows, each in its own unit and never summed across units: whole
// credits (with the mark and a screen-reader unit), and MANA in exact token units with its network.

import ManaMark from "../../../atoms/ManaMark";
import { isPolygon } from "../../model/chain";
import { formatMana, type ManaWei } from "../../model/money";
import { creditsUnit, formatCreditsFull } from "../../checkout/lib/currency";
import CreditMark from "./CreditMark";
import "./amount.css";

export function CreditsAmount({ credits, size = 14, className = "" }: { credits: number; size?: number; className?: string }) {
  return (
    <span className={`shop-amount shop-amount--credits ${className}`.trim()}>
      <CreditMark size={size} />
      <span className="shop-amount__value">{formatCreditsFull(credits)}</span>
      <span className="shop-amount__unit">{creditsUnit(credits)}</span>
    </span>
  );
}

export function ManaAmount({ wei, chainId, size = 14, className = "" }: { wei: bigint; chainId?: number; size?: number; className?: string }) {
  return (
    <span className={`shop-amount shop-amount--mana ${className}`.trim()}>
      <ManaMark size={size} network={chainId !== undefined && isPolygon(chainId) ? "polygon" : "ethereum"} />
      <span className="shop-amount__value">{formatMana(wei as ManaWei, { group: true })}</span>
      <span className="shop-amount__unit">MANA</span>
    </span>
  );
}
