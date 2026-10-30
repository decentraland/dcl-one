// from: shop@94584a5c:app/src/components/PaymentMethodStep/PaymentMethodStep.tsx, app/src/components/PaymentCtas/PaymentCtas.tsx;
//       supersedes v2 ui3/src/marketplace/components/MkPaymentSection.tsx
import { useId } from "react";

import { outageText, reasonText } from "../../payment/copy";
import type { PaymentOptionView, PaymentPresentation } from "../../payment/present";
import type { PaymentMethod } from "../../model/settlement";
import { chainLabel } from "../../model/chain";
import { creditsCeil, creditsFloor, mulCents, mulMana } from "../../model/money";
import { CreditsAmount, ManaAmount } from "../../credits/components/Amount";
import "./paymentchoice.css";

function stateNote(option: PaymentOptionView): string | null {
  switch (option.state.kind) {
    case "ready":
      return null;
    case "signInRequired":
      return "Sign in to pay with Credits.";
    case "topUpRequired": {
      const short = creditsCeil(option.state.short);
      return `Your balance is ${short} ${short === 1 ? "Credit" : "Credits"} short. Top up to pay with Credits.`;
    }
    case "insufficientMana":
      return "Your wallet doesn't hold enough MANA for this.";
    case "walletRequired":
      return "Needs a web3 wallet.";
    case "unavailable":
      return outageText(option.method, option.state.missing, option.state.withheld);
  }
}

function Price({ option, quantity, chainId }: { option: PaymentOptionView; quantity: number; chainId: number }) {
  if (option.method === "credits") {
    const each = option.charge;
    const total = each === null ? null : mulCents(each, quantity);
    return total === null ? <span className="shop-pay__na">Price unavailable</span> : <CreditsAmount credits={creditsFloor(total)} />;
  }
  const q = option.quote?.method === "mana" ? option.quote.amount : null;
  const total = q === null ? null : mulMana(q, quantity);
  return total === null ? <span className="shop-pay__na">Price unavailable</span> : <ManaAmount wei={total} chainId={chainId} />;
}

export type PaymentChoiceProps = {
  view: PaymentPresentation;
  selected: PaymentMethod | null;
  onSelect(method: PaymentMethod): void;
  quantity: number;
  manaChainId: number;
  disabled?: boolean;
  onRetry?(): void;
  label: string;
  hidePrice?: boolean;
};

export default function PaymentChoice({ view, selected, onSelect, quantity, manaChainId, disabled = false, onRetry, label, hidePrice = false }: PaymentChoiceProps) {
  const name = useId();
  const current = selected ?? view.primary;
  const hasOutage = view.options.some((o) => o.state.kind === "unavailable");

  if (view.manaOnly) {
    const mana = view.options.find((o) => o.method === "mana");
    return (
      <div className="shop-pay shop-pay--mana-only" data-testid="payment-mana-only">
        <div className="shop-pay__row">
          <span className="shop-pay__tag">Paid in MANA on {chainLabel(manaChainId)}</span>
          {hidePrice ? null : mana ? <Price option={mana} quantity={quantity} chainId={manaChainId} /> : <span className="shop-pay__na">Price unavailable</span>}
        </div>
        {view.reasons.length > 0 ? <p className="shop-pay__why">{view.reasons.map(reasonText).join(" ")}</p> : null}
        {mana && stateNote(mana) ? <p className="shop-pay__note">{stateNote(mana)}</p> : null}
        {hasOutage && onRetry ? (
          <button type="button" className="shop-pay__retry" onClick={onRetry}>
            Check the price again
          </button>
        ) : null}
      </div>
    );
  }

  return (
    <fieldset className="shop-pay" disabled={disabled} data-testid="payment-choice">
      <legend className="shop-pay__legend">Pay {label} with</legend>
      {view.options.map((option) => {
        const note = stateNote(option);
        return (
          <label key={option.method} className="shop-pay__option" data-method={option.method} data-state={option.state.kind} data-selected={current === option.method || undefined}>
            <input type="radio" name={name} value={option.method} checked={current === option.method} onChange={() => onSelect(option.method)} />
            <span className="shop-pay__method">{option.method === "credits" ? "Credits" : `MANA on ${chainLabel(manaChainId)}`}</span>
            {hidePrice ? <span /> : <Price option={option} quantity={quantity} chainId={manaChainId} />}
            {note ? <span className="shop-pay__note">{note}</span> : null}
            {option.method === "mana" && option.mixed ? (
              <span className="shop-pay__mixed">
                Or {option.mixed.credits} Credits and the rest in MANA, from the item page.
              </span>
            ) : null}
          </label>
        );
      })}
      {hasOutage && onRetry ? (
        <button type="button" className="shop-pay__retry" onClick={onRetry}>
          Check the price again
        </button>
      ) : null}
    </fieldset>
  );
}
