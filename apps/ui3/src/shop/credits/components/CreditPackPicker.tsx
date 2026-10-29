// from: shop@94584a5c:app/src/components/CreditPackPicker/CreditPackPicker.tsx
// The no-funds pack tiles and the running total, shared by every screen that sells a top-up
// mid-purchase. `recommendedId` is a promise that the pack finishes the purchase: pass it only when
// `offerablePacks` reports `closesGap`.

import { formatCredits } from "../../checkout/lib/currency";
import type { CreditPack } from "../payments";
import CreditMark from "./CreditMark";
import "./packpicker.css";

export default function CreditPackPicker({
  packs,
  selectedId,
  onSelect,
  recommendedId,
}: {
  packs: CreditPack[];
  selectedId?: string;
  onSelect: (id: string) => void;
  recommendedId?: string;
}) {
  const selected = packs.find((p) => p.id === selectedId);
  return (
    <div className="shop-packpicker">
      <div className="shop-packpicker__grid" role="group" aria-label="Credit packs" data-testid="credit-packs">
        {packs.map((p) => (
          <button
            key={p.id}
            type="button"
            className="shop-packpicker__tile"
            data-testid="credit-pack"
            data-on={p.id === selectedId || undefined}
            aria-pressed={p.id === selectedId}
            aria-label={`${formatCredits(p.credits)} Credits for $${p.usd.toFixed(2)}${p.id === recommendedId ? ", recommended" : ""}`}
            onClick={() => onSelect(p.id)}
          >
            {p.id === recommendedId ? (
              <span className="shop-packpicker__badge" data-testid="pack-recommended" aria-hidden="true">
                Recommended
              </span>
            ) : null}
            <CreditMark size={22} />
            <span className="shop-packpicker__amount">{formatCredits(p.credits)}</span>
            <span className="shop-packpicker__usd">${p.usd.toFixed(2)}</span>
          </button>
        ))}
      </div>
      <div className="shop-packpicker__total">
        <span>
          <CreditMark size={14} /> <b data-testid="topup-total-credits">{formatCredits(selected?.credits ?? 0)}</b> Credits
        </span>
        <span>${(selected?.usd ?? 0).toFixed(2)}</span>
      </div>
    </div>
  );
}
