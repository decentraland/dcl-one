// from: shop@3a322cf9:app/src/components/CartCheckoutModal/CartCheckoutModal.tsx, app/src/pages/Cart.tsx (openNoFunds,
//       buyCreditsAndItems, the processing stages, the failure), app/src/intl/en.json (cartCheckout)
// The cart's checkout dialog, upstream's CartCheckoutModal over the shell Buy now's dialog shares (CheckoutDialog.tsx).
// The cart page decides when it shows and what it says; the dialog holds no purchase logic of its own:
//   - "Buy Credits and Items" when the Credits fall short: the shortfall, the cart's lines, the packs that finish the
//     purchase and the Stripe hand-off, which comes back to the cart;
//   - "Buy Items" while the purchase runs: preparing it, then completing it once it is sent;
//   - the failure panel, with a retry.
// Purchase complete is the success page the cart navigates to, as upstream's.
import { createPortal } from "react-dom";

import type { CartLineDisplay } from "@shop/model/purchase";
import { DialogAsset, DialogAssetList, DialogHead, DialogShell, ErrorBody, NoFundsBody, ProcessingBody, useDialog } from "./CheckoutDialog";

const TEXT = {
  label: "Check out your cart",
  titleBuy: "Buy Items",
  titleNoFunds: "Buy Credits and Items",
  titleError: "Oops! Something went wrong",
  preparing: "Preparing your purchase…",
  completing: "Completing your purchase…",
} as const;

/** A cart line as the dialog lists it: what it shows, its whole-credit price per unit and how many units. */
export type CheckoutLine = { id: string; display: CartLineDisplay; priceCredits: number; quantity: number };

export type CartCheckoutPhase =
  /** The Credits fall short by `shortfall` whole credits. */
  | { kind: "nofunds"; shortfall: number }
  /** Preparing (the Credits reserved, the purchase signed) or completing (sent, being confirmed). */
  | { kind: "processing"; stage: "preparing" | "completing" }
  | { kind: "error"; message: string | null };

export default function CartCheckoutModal(props: {
  phase: CartCheckoutPhase;
  lines: CheckoutLine[];
  /** The Credits balance as the head shows it. */
  balance: string;
  onClose(): void;
  /** Runs again what did not complete; absent when nothing can run now (the cart's banner says why). */
  onRetry?: () => void;
  /** The cart's top-up hand-off (useCheckout's stashForTopUp), left before the pack checkout opens. */
  onLeave(): void;
  /** The pack checkout did not open: the hand-off it left is dropped. */
  onLeaveFailed(): void;
  onCheckout(session: { orderId: string; url?: string }): void;
}) {
  const { phase } = props;
  const busy = phase.kind === "processing";
  const close = () => {
    if (!busy) props.onClose();
  };
  const dialog = useDialog(busy, close);
  if (typeof document === "undefined") return null;
  const title = phase.kind === "error" ? TEXT.titleError : phase.kind === "nofunds" ? TEXT.titleNoFunds : TEXT.titleBuy;
  const units = props.lines.reduce((n, l) => n + l.quantity, 0);
  return createPortal(
    <DialogShell dialog={dialog} label={TEXT.label} testId="cart-checkout" busy={busy} onClose={close} tall={busy} phase={phase.kind}>
      <DialogHead title={title} balance={props.balance} busy={busy} onClose={close} testId="cart-checkout" />
      {phase.kind === "nofunds" ? (
        <NoFundsBody
          testId="cart-checkout"
          shortfall={phase.shortfall}
          units={units}
          learnMoreBelow
          items={
            <DialogAssetList>
              {props.lines.map((l) => (
                <DialogAsset key={l.id} display={l.display} priceCredits={l.priceCredits} quantity={l.quantity} flapTestId="cart-checkout-flap" />
              ))}
            </DialogAssetList>
          }
          onLeave={() => {
            props.onLeave();
            return null;
          }}
          onFailed={() => props.onLeaveFailed()}
          onCancel={close}
          onCheckout={props.onCheckout}
        />
      ) : null}
      {phase.kind === "processing" ? (
        <ProcessingBody text={phase.stage === "preparing" ? TEXT.preparing : TEXT.completing} />
      ) : null}
      {phase.kind === "error" ? <ErrorBody message={phase.message} onClose={close} onRetry={props.onRetry} /> : null}
    </DialogShell>,
    document.body,
  );
}
