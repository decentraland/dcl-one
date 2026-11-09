// from: shop@94584a5c:app/src/components/CartCheckoutModal/CartCheckoutModal.tsx (the no-funds phase),
//       app/src/pages/Cart.tsx (buyCreditsAndItems)
// The explicit top-up a short credits payment leads to (SHOP.md: insufficient credits lead to a
// top-up, never an automatic switch to MANA). Offers only packs that finish the purchase, recommends
// the cheapest one that does, and leaves for the pack checkout keeping a hand-off so the purchase
// resumes when the credits land. Inside the iOS app's web view credits are sold by the app instead.

import { useMemo, useState } from "react";

import Button from "../../../atoms/Button";
import { creditsCeil, type UsdCents } from "../../model/money";
import { friendlyError } from "../../checkout/lib/errors";
import { useShopServices } from "../../checkout/services";
import { useCreditPacks } from "../hooks";
import { isIapMode } from "../iap";
import { createPackCheckout, MAX_OFFER_PACKS, offerablePacks } from "../payments";
import CreditPackPicker from "./CreditPackPicker";
import CreditRate from "./CreditRate";
import "./topup.css";

export type TopUpPanelProps = {
  /** What the credits payment is short by, in cents (rounded up to whole credits here). */
  shortCents: UsdCents;
  /** Leaves for the pack checkout; `orderId` is where a checkout without a hosted page resumes. */
  onCheckout?(session: { orderId: string; url?: string }): void;
  /** Called right before leaving, to stash the purchase this top-up finishes. */
  onBeforeLeave?(): void;
  onCancel?(): void;
};

export default function TopUpPanel({ shortCents, onCheckout, onBeforeLeave, onCancel }: TopUpPanelProps) {
  const { client, openExternal } = useShopServices();
  const { packs } = useCreditPacks();
  const shortCredits = creditsCeil(shortCents);
  const offer = useMemo(() => offerablePacks(packs.slice(0, MAX_OFFER_PACKS), shortCredits), [packs, shortCredits]);
  const [selected, setSelected] = useState<string | undefined>(offer.recommended?.id);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (isIapMode()) {
    return (
      <section className="shop-topup" aria-label="Top up Credits" data-testid="topup-panel">
        <h3 className="shop-topup__title">You need {shortCredits} more Credits</h3>
        <p>Get Credits in the Decentraland app, then come back to finish this purchase.</p>
      </section>
    );
  }

  async function buy() {
    if (!selected) return;
    setBusy(true);
    setError(null);
    try {
      onBeforeLeave?.();
      const session = await createPackCheckout(selected, client);
      if (onCheckout) onCheckout(session);
      else if (session.url) openExternal(session.url);
    } catch (e) {
      setError(friendlyError(e, "We couldn't start the Credits checkout. Try again."));
      setBusy(false);
    }
  }

  const pack = offer.packs.find((p) => p.id === selected);
  return (
    <section className="shop-topup" aria-label="Top up Credits" data-testid="topup-panel">
      <h3 className="shop-topup__title">You need {shortCredits} more Credits</h3>
      <p className="shop-topup__lead">
        {offer.closesGap ? "Any of these packs covers what's missing. Your purchase continues when the Credits arrive." : "No single pack covers it; the largest gets you closest."}
      </p>
      <CreditPackPicker packs={offer.packs} selectedId={selected} onSelect={setSelected} recommendedId={offer.closesGap ? offer.recommended?.id : undefined} />
      <CreditRate />
      {error ? (
        <p className="shop-topup__error" role="alert">
          {error}
        </p>
      ) : null}
      <div className="shop-topup__actions">
        {onCancel ? (
          <Button variant="ghost" onClick={onCancel} disabled={busy}>
            Not now
          </Button>
        ) : null}
        <Button onClick={() => void buy()} disabled={busy || !pack} data-testid="topup-buy">
          {busy ? "Opening checkout…" : pack ? `Buy ${pack.credits} Credits for $${pack.usd.toFixed(2)}` : "Choose a pack"}
        </Button>
      </div>
    </section>
  );
}
