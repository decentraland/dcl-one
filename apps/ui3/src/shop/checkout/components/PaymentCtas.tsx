// from: shop@3a322cf9:app/src/components/PaymentCtas/PaymentCtas.tsx
// The purchase CTAs, one per payment rail the buyer's balances support. Each button IS the payment: the
// credits rail (the buy gradient), the MANA rail (dark, with its amount). When no rail covers the
// purchase, the credits fallback stays on screen and opens the flow that offers a top-up, so the way to
// buy more credits never depends on the MANA balance. MANA the buyer holds that cannot pay is shown on a
// disabled button with its reason, never hidden.
import { useId } from "react";

import ManaMark from "../../../atoms/ManaMark";
import { isEthereum } from "../../model/chain";
import { formatMana } from "../../model/money";
import type { CartRail } from "../lib/cart-rails";
import styles from "./PaymentCtas.module.css";

export type PaymentCtasProps = {
  rails: readonly CartRail[];
  /** The credits button's label ("Checkout" alone, "Buy with Credits" beside another rail). */
  creditsLabel: string;
  /** Shown when no rail covers the purchase: the credits path, opening a top-up. */
  creditsFallback: { label: string; onClick(): void } | null;
  /** Held MANA that cannot pay: the disabled MANA button. */
  manaShortfall: { manaWei: bigint; chainId: number } | null;
  busy?: boolean;
  /** A payment the buttons started is under way: every button says so to assistive tech. */
  working?: boolean;
  onPay(rail: CartRail): void;
};

function ManaAmountMark({ wei, chainId }: { wei: bigint; chainId: number }) {
  return (
    <span className={styles.amount}>
      <span className={styles.manaMark}>
        <ManaMark size={24} network={isEthereum(chainId) ? "ethereum" : "polygon"} />
      </span>
      <span>{formatMana(wei as never)}</span>
    </span>
  );
}

export default function PaymentCtas({ rails, creditsLabel, creditsFallback, manaShortfall, busy = false, working = false, onPay }: PaymentCtasProps) {
  const hasCredits = rails.some((r) => r.method === "credits");
  const noteId = useId();
  return (
    <div className={styles.root} aria-busy={working || undefined}>
      {!hasCredits && creditsFallback ? (
        <button type="button" className={styles.btn} data-variant="credits" data-testid="pay-with-credits-topup" onClick={creditsFallback.onClick} disabled={busy}>
          <span>{creditsFallback.label}</span>
        </button>
      ) : null}
      {rails.map((rail) =>
        rail.method === "credits" ? (
          <button key="credits" type="button" className={styles.btn} data-variant="credits" data-testid="pay-with-credits" onClick={() => onPay(rail)} disabled={busy}>
            <span>{creditsLabel}</span>
          </button>
        ) : (
          <button key="mana" type="button" className={styles.btn} data-variant="mana" data-testid="pay-with-mana" onClick={() => onPay(rail)} disabled={busy}>
            <span>Buy with MANA</span>
            <ManaAmountMark wei={rail.manaWei} chainId={rail.chainId} />
          </button>
        ),
      )}
      {manaShortfall ? (
        <>
          {/* As upstream: the MANA the wallet holds, on a disabled button, with the reason it cannot pay. */}
          <button type="button" className={styles.btn} data-variant="mana" data-testid="pay-with-mana-disabled" disabled aria-describedby={noteId}>
            <span>Buy with MANA</span>
            <ManaAmountMark wei={manaShortfall.manaWei} chainId={manaShortfall.chainId} />
          </button>
          <p className={styles.note} id={noteId} data-testid="mana-shortfall-note">
            Not enough MANA
          </p>
        </>
      ) : null}
    </div>
  );
}
