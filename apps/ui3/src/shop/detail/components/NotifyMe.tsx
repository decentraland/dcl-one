// from: shop@2d3bf62:app/src/components/NotifyMe/NotifyMe.tsx, NotifyMe.styles.ts

import { useState, type FormEvent } from "react";

import { shopClient } from "../../api/shop";
import Icon from "../../discovery/components/Icon";
import { useShopQuery } from "../../discovery/query";
import { useShopWallet } from "../../wallet/context";
import { pdpText } from "../copy";

const isEmailish = (v: string) => /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(v.trim());

export default function NotifyMe({ contractAddress, itemId, chainId }: { contractAddress: string; itemId: string; chainId: number }) {
  const wallet = useShopWallet();
  const signedIn = wallet.session.kind !== "signedOut" && !!wallet.account;
  const [email, setEmail] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [submitted, setSubmitted] = useState(false);
  const [error, setError] = useState(false);
  const status = useShopQuery(
    signedIn ? ["notify-request", contractAddress, itemId, wallet.account] : null,
    (signal) => shopClient().notify.list<{ data?: { subscribed?: boolean } } | { subscribed?: boolean }>({ contractAddress, itemId }, { signal }),
    { staleMs: 60_000 },
  );
  const body = status.data as { data?: { subscribed?: boolean }; subscribed?: boolean } | undefined;
  const subscribed = submitted || !!(body?.subscribed ?? body?.data?.subscribed);

  if (!signedIn) {
    return wallet.host?.onSignIn ? (
      <button type="button" className="shop-notify__btn" onClick={wallet.host.onSignIn} data-testid="notify-signin">
        {pdpText("notifyMe.signInCta")}
      </button>
    ) : null;
  }
  if (subscribed) {
    return (
      <p className="shop-notify__done" data-testid="notify-subscribed">
        <span className="shop-notify__check" aria-hidden>
          <Icon name="check" size={12} />
        </span>
        {pdpText("notifyMe.subscribedBody")}
      </p>
    );
  }
  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    if (!isEmailish(email) || submitting) return;
    setSubmitting(true);
    setError(false);
    try {
      await shopClient().notify.create({ contractAddress, itemId, chainId, email: email.trim() });
      setSubmitted(true);
    } catch {
      setError(true);
    } finally {
      setSubmitting(false);
    }
  }
  return (
    <form className="shop-notify" onSubmit={(e) => void onSubmit(e)} data-testid="notify">
      <label className="shop-notify__field">
        <span className="shop-sr">{pdpText("notifyMe.emailLabel")}</span>
        <input type="email" required value={email} placeholder={pdpText("notifyMe.emailPlaceholder")} onChange={(e) => setEmail(e.target.value)} />
      </label>
      <button type="submit" className="shop-notify__btn" disabled={submitting || !isEmailish(email)}>
        {pdpText("notifyMe.cta")}
      </button>
      {error ? (
        <p className="shop-notify__error" role="alert">
          {pdpText("notifyMe.error")}
        </p>
      ) : null}
    </form>
  );
}
