// The web2 MANA guidance (SHOP.md): for a buyer without a web3 wallet on a MANA-only purchase, say the
// purchase is paid in MANA and why, show the network, the exact balance required and any gas, and
// offer a clear connect/setup path. The signed-in account's own wallet comes first; a different
// wallet is an explicit account switch that names the address the item will go to. The item and the
// return destination are kept across the switch, and the purchase is revalidated afterwards. Nothing
// here suggests credits.

import { useId, useState } from "react";

import Button from "../../atoms/Button";
import { shortAddress } from "../model/address";
import { chainLabel, gasTokenSymbol } from "../model/chain";
import { formatMana, type ManaWei } from "../model/money";
import type { Reason, WalletGuidance } from "../model/settlement";
import { reasonText } from "../payment/copy";
import type { CartAddInput } from "../cart/store";
import { stashWalletReturn } from "./returnTo";
import "./managuidance.css";

export type GuidanceRevalidation = {
  status: "checking" | "current" | "changed" | "failed";
  onRetry(): void;
};

export type ManaGuidanceProps = {
  guidance: WalletGuidance;
  /** The server's MANA quote for this purchase, in exact units; null while unavailable. */
  required: ManaWei | null;
  /** What to say instead of "unavailable" when the buyer sets the amount (a bid): `required` is then null. */
  requiredNote?: string;
  reasons: Reason[];
  item: { name: string; href: string };
  /** Where to come back after connecting or switching. */
  returnTo: string;
  /** The cart line this guidance is for: brought to the account the buyer switches to. */
  carry?: CartAddInput;
  /** The wallet as the session sees it, once one is connected. */
  wallet?: { account: string | null; chainId: number | null; manaBalance?: ManaWei | null; pendingChainId?: number | null };
  onSignIn?(): void;
  onConnectWallet?(): void;
  onSwitchAccount?(): void;
  onSwitchNetwork?(chainId: number): void;
  /** A page on setting up a wallet. */
  setupUrl?: string;
  revalidation?: GuidanceRevalidation | null;
};

export default function ManaGuidance({
  guidance,
  required,
  requiredNote,
  reasons,
  item,
  returnTo,
  carry,
  wallet,
  onSignIn,
  onConnectWallet,
  onSwitchAccount,
  onSwitchNetwork,
  setupUrl,
  revalidation,
}: ManaGuidanceProps) {
  const headingId = useId();
  const [confirmSwitch, setConfirmSwitch] = useState(false);
  const network = chainLabel(guidance.chainId);
  const amount = required === null ? null : formatMana(required, { group: true });
  const keep = () => stashWalletReturn({ itemName: item.name, itemHref: item.href, returnTo, fromAccount: guidance.account, ...(carry ? { carry: [carry] } : {}) });
  const switchedFrom = guidance.account && wallet?.account && wallet.account !== guidance.account ? guidance.account : null;
  const wrongNetwork = wallet?.account && wallet.chainId !== null && wallet.chainId !== guidance.chainId;
  const short = wallet?.manaBalance != null && required !== null && wallet.manaBalance < required;

  return (
    <section className="shop-guidance" aria-labelledby={headingId}>
      <h3 id={headingId} className="shop-guidance__title">
        This purchase is paid in MANA
      </h3>
      {reasons.length > 0 && (
        <ul className="shop-guidance__reasons">
          {reasons.map((r) => (
            <li key={r}>{reasonText(r)}</li>
          ))}
        </ul>
      )}

      <dl className="shop-guidance__needs">
        <div>
          <dt>Network</dt>
          <dd>{network}</dd>
        </div>
        <div>
          <dt>Balance needed</dt>
          <dd>{amount === null ? (requiredNote ?? "The MANA price is unavailable right now.") : `${amount} MANA on ${network}`}</dd>
        </div>
        <div>
          <dt>Network fee</dt>
          <dd>
            {guidance.nativeGasRequired
              ? `Some ${gasTokenSymbol(guidance.chainId)} on ${network} to pay the network fee (gas).`
              : `No ${gasTokenSymbol(guidance.chainId)} needed: ${network} purchases are relayed.`}
          </dd>
        </div>
      </dl>

      <div className="shop-guidance__wallet">
        {guidance.account === null ? (
          <>
            <p>Sign in with a web3 wallet that holds MANA on {network} to buy this.</p>
            <div className="shop-guidance__actions">
              <Button
                size="sm"
                onClick={() => {
                  keep();
                  onSignIn?.();
                }}
              >
                Sign in with a wallet
              </Button>
            </div>
          </>
        ) : (
          <>
            <p>
              You are signed in as <span className="shop-guidance__address">{shortAddress(guidance.account)}</span> without a
              web3 wallet in this session.
            </p>
            <div className="shop-guidance__actions">
              <Button
                size="sm"
                onClick={() => {
                  keep();
                  onConnectWallet?.();
                }}
              >
                Connect the wallet for {shortAddress(guidance.account)}
              </Button>
              {!confirmSwitch && (
                <Button size="sm" variant="ghost" onClick={() => setConfirmSwitch(true)}>
                  Use a different wallet
                </Button>
              )}
            </div>
            {confirmSwitch && (
              <div className="shop-guidance__switch" role="group" aria-label="Switch account">
                <p>
                  Connecting a different wallet signs you in as that wallet&apos;s account. {item.name} will be sent to the
                  connected wallet&apos;s address, not to {shortAddress(guidance.account)}.
                </p>
                <div className="shop-guidance__actions">
                  <Button
                    size="sm"
                    onClick={() => {
                      keep();
                      onSwitchAccount?.();
                    }}
                  >
                    Switch to another wallet
                  </Button>
                  <Button size="sm" variant="ghost" onClick={() => setConfirmSwitch(false)}>
                    Keep {shortAddress(guidance.account)}
                  </Button>
                </div>
              </div>
            )}
          </>
        )}
        {setupUrl && (
          <p className="shop-guidance__setup">
            New to wallets?{" "}
            <a href={setupUrl} target="_blank" rel="noopener noreferrer">
              Set up a wallet
            </a>
          </p>
        )}
      </div>

      {switchedFrom && wallet?.account && (
        <p className="shop-guidance__notice" role="status">
          You switched from {shortAddress(switchedFrom)} to {shortAddress(wallet.account)}. {item.name} will be received by{" "}
          {shortAddress(wallet.account)}.
        </p>
      )}
      {wrongNetwork && wallet?.chainId != null && (
        <div className="shop-guidance__notice" role="status">
          <p>
            Your wallet is on {chainLabel(wallet.chainId)}. This purchase runs on {network}.
          </p>
          <Button
            size="sm"
            variant="secondary"
            disabled={wallet.pendingChainId != null}
            onClick={() => onSwitchNetwork?.(guidance.chainId)}
          >
            {wallet.pendingChainId != null ? "Confirm in your wallet…" : `Switch to ${network}`}
          </Button>
        </div>
      )}
      {short && wallet?.manaBalance != null && (
        <p className="shop-guidance__notice" role="status">
          Your wallet holds {formatMana(wallet.manaBalance, { group: true })} MANA on {network}; this purchase needs {amount} MANA.
        </p>
      )}

      {revalidation && <RevalidationNote revalidation={revalidation} />}

      <p className="shop-guidance__return">
        You will come back to <a href={item.href}>{item.name}</a>.
      </p>
    </section>
  );
}

function RevalidationNote({ revalidation }: { revalidation: GuidanceRevalidation }) {
  switch (revalidation.status) {
    case "checking":
      return (
        <p className="shop-guidance__notice" role="status" aria-live="polite">
          Checking the price, availability and ownership again for this wallet…
        </p>
      );
    case "current":
      return (
        <p className="shop-guidance__notice" role="status">
          Price and availability confirmed for this wallet.
        </p>
      );
    case "changed":
      return (
        <p className="shop-guidance__notice" role="alert">
          The price or availability changed after the switch. Review it before buying.
        </p>
      );
    case "failed":
      return (
        <div className="shop-guidance__notice" role="alert">
          <p>We could not check the price again.</p>
          <Button size="sm" variant="secondary" onClick={revalidation.onRetry}>
            Try again
          </Button>
        </div>
      );
  }
}
