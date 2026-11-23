import { isIdentityExpired } from "./expiry";
import {
  getIdentity,
  getSavedWebsiteIdentity,
  setBrowserWalletIdentity,
  subscribe,
  suspendWalletIdentity,
} from "./session";
import { suspendSignedInAccount, switchSignedInAccount } from "./session-change";
import { browserWalletRdnsFor, recordSignIn, UNRECORDED_BROWSER_WALLET } from "./signin-kind";
import { sharedIdentityFor, walletSessionFor } from "./single-sign-on";
import { browserWalletProvider, selectWallet } from "./wallet";

const ADDRESS = /^0x[\da-f]{40}$/i;
const ANNOUNCE_PROVIDER = "eip6963:announceProvider";

type WalletAccountChange = "activated" | "sign-in";

function exposedAccount(value: unknown): string | null | undefined {
  if (!Array.isArray(value)) return undefined;
  if (value.length === 0) return null;
  const first: unknown = value[0];
  return typeof first === "string" && ADDRESS.test(first) ? first.toLowerCase() : undefined;
}

export function watchBrowserWalletAccount(win: Window, onChange: (change: WalletAccountChange) => void): () => void {
  let boundSigner: string | undefined;
  let boundRdns: string | undefined;
  let unbind: (() => void) | undefined;

  function bind() {
    const identity = getSavedWebsiteIdentity();
    const signer = identity?.signer.toLowerCase();
    const recordedRdns = signer ? browserWalletRdnsFor(signer) : null;
    const rdns = recordedRdns === UNRECORDED_BROWSER_WALLET ? "injected" : recordedRdns;
    if (signer && signer === boundSigner && rdns === boundRdns) return;
    unbind?.();
    unbind = undefined;
    boundSigner = undefined;
    boundRdns = undefined;
    if (!signer || !rdns) return;
    try {
      const provider = browserWalletProvider(rdns);
      if (!provider) return;
      const { on, removeListener } = provider;
      if (typeof on !== "function" || typeof removeListener !== "function") return;

      let live = true;
      let sawEvent = false;
      let trusted = recordedRdns !== UNRECORDED_BROWSER_WALLET;
      let pendingAccount: string | null = null;
      // A suspended session is signed out here: the verified-wallet cookie (routes/auth.callback.tsx) must not
      // keep naming it while the wallet shows another account, or none.
      const suspend = () => {
        const active = getIdentity();
        suspendWalletIdentity();
        if (active) suspendSignedInAccount();
      };
      const apply = (accounts: unknown) => {
        if (!live || !trusted) return;
        const account = exposedAccount(accounts);
        if (account === undefined) return;
        if (account === null) {
          pendingAccount = null;
          suspend();
          return;
        }
        const saved = getSavedWebsiteIdentity();
        if (saved?.signer.toLowerCase() !== signer) return;
        if (account === signer && getIdentity() && !isIdentityExpired(saved)) return;

        const next = account === signer && !isIdentityExpired(saved)
          ? saved
          : sharedIdentityFor(account) ?? walletSessionFor(account);
        if (next) {
          setBrowserWalletIdentity(next, rdns, true);
          // Another account, or this one back from a suspension that cleared its cookie: the server names it again.
          void switchSignedInAccount(next);
          onChange("activated");
        } else if (pendingAccount !== account) {
          pendingAccount = account;
          suspend();
          onChange("sign-in");
        }
      };
      const changed = (accounts: unknown) => {
        if (exposedAccount(accounts) !== undefined) sawEvent = true;
        apply(accounts);
      };
      on.call(provider, "accountsChanged", changed);
      boundSigner = signer;
      boundRdns = rdns;
      unbind = () => {
        live = false;
        try {
          removeListener.call(provider, "accountsChanged", changed);
        } catch {
          // An extension can revoke access to its provider while the page is open.
        }
      };
      if (trusted) selectWallet(rdns);
      void Promise.resolve()
        .then(() => live && !sawEvent ? provider.request({ method: "eth_accounts" }) : undefined)
        .then((accounts) => {
          if (!live || sawEvent) return;
          // Older Web3 sessions can come from a phone. Require a matching account before trusting the default wallet.
          if (!trusted && exposedAccount(accounts) === signer) {
            trusted = true;
            recordSignIn(signer, "web3", rdns);
            selectWallet(rdns);
          }
          if (exposedAccount(accounts)) apply(accounts);
        })
        .catch(() => {});
    } catch {
      // Some extensions throw even when reading the injected provider's event methods.
    }
  }

  const unsubscribe = subscribe(bind);
  bind();
  win.addEventListener(ANNOUNCE_PROVIDER, bind);
  return () => {
    win.removeEventListener(ANNOUNCE_PROVIDER, bind);
    unsubscribe();
    unbind?.();
  };
}
