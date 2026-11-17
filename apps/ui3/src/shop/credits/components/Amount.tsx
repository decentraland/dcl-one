// The two money figures the Shop shows, each in its own unit and never summed across units: whole
// credits (with the mark and a screen-reader unit), and MANA to two decimals, rounded up, with its
// network (the exact amount in its title).

import ManaMark from "../../../atoms/ManaMark";
import { isPolygon } from "@shop/model/chain";
import { formatMana, formatManaPrice, type ManaWei } from "@shop/model/money";
import { creditsUnit, formatCreditsFull } from "@shop/checkout/lib/currency";
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
      <span className="shop-amount__value" title={`${formatMana(wei as ManaWei, { group: true })} MANA`}>
        {formatManaPrice(wei as ManaWei)}
      </span>
      <span className="shop-amount__unit">MANA</span>
    </span>
  );
}
