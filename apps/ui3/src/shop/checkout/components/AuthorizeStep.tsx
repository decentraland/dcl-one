// from: shop@94584a5c:app/src/components/AuthorizeStep/AuthorizeStep.tsx
// The explicit approval a self-custody wallet grants before a MANA purchase: which contract may spend
// the buyer's MANA and why, counted when a basket needs several ("1 of 2"). The grant runs inside the
// buyer's click (wallets refuse unattributed wallet requests). A managed wallet never sees this.

import { useState } from "react";

import Button from "../../../atoms/Button";
import Modal from "../../../components/Modal";
import { shortAddress } from "../../model/address";
import { chainLabel } from "../../model/chain";
import { formatMana, type ManaWei } from "../../model/money";
import { contractAt } from "../chain/contracts";
import type { ShopAuthorization } from "../lib/authorizations";
import { friendlyError, isRejection } from "../lib/errors";
import "./authorizestep.css";

export type AuthorizeStepProps = {
  auth: ShopAuthorization;
  step?: { index: number; total: number };
  requiredWei?: bigint;
  /** Grants the approval (lib/authorizations setAuthorization); throws on a declined prompt or a failure. */
  grant(): Promise<void>;
  onAuthorized(): void;
  onCancel(): void;
  portal?: boolean;
};

export default function AuthorizeStep({ auth, step, requiredWei, grant, onAuthorized, onCancel, portal }: AuthorizeStepProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const spender = contractAt(auth.chainId, auth.spenderAddress);
  const spenderName = spender?.label ?? shortAddress(auth.spenderAddress);

  async function authorize() {
    setBusy(true);
    setError(null);
    try {
      await grant();
      onAuthorized();
    } catch (e) {
      setError(isRejection(e) ? "You declined the approval in your wallet." : friendlyError(e, "The approval didn't go through. Try again."));
      setBusy(false);
    }
  }

  return (
    <Modal onClose={busy ? undefined : onCancel} ariaLabel="Approve MANA spending" portal={portal} width={440}>
      <div className="shop-authorize" data-testid="authorize-step">
        <div className="shop-authorize__head">
          <h2>Approve MANA spending</h2>
          {step && step.total > 1 ? (
            <span className="shop-authorize__count" data-testid="authorize-step-count">
              {step.index} of {step.total}
            </span>
          ) : null}
        </div>
        <p>
          To buy with MANA, let <b>{spenderName}</b> on {chainLabel(auth.chainId)} move MANA from your wallet
          {requiredWei ? (
            <>
              {" "}
              (this purchase needs <b>{formatMana(requiredWei as ManaWei, { group: true })} MANA</b>)
            </>
          ) : null}
          . Your wallet asks you to confirm it once.
        </p>
        <p className="shop-authorize__note">You can revoke it any time from the Shop settings.</p>
        {error ? (
          <p className="shop-authorize__error" role="alert">
            {error}
          </p>
        ) : null}
        <div className="shop-authorize__actions">
          <Button variant="ghost" onClick={onCancel} disabled={busy}>
            Cancel
          </Button>
          <Button onClick={() => void authorize()} disabled={busy} data-testid="authorize-step-action">
            {busy ? "Confirm in your wallet…" : "Approve"}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
