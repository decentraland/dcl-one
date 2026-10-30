import { CREATOR_HUB_HOME, CREATOR_HUB_SCOPE, creatorHubHost, type CreatorHubHost } from "@ui/creatorhub/explorer-host";
import { embedEscapeHref, installEmbedGuards } from "@ui/web/frames/embed";

import { claimWalletCookieOnUse, setHostIdentity } from "@/data/lib/auth/session";
import type { AuthIdentity } from "@/data/lib/auth/types";
import { installHostWalletProvider } from "@/data/lib/auth/wallet";
import { installHostSignIn } from "../auth/signin-store";

// A document framed by the Explorer's Create panel (ui3/src/creatorhub/explorer-host.ts, version 1)
// runs on the Explorer's session: its identity, sign-in and wallet, never the website's own login.

let adopted: CreatorHubHost | null = null;

/** The identity arrives from the Explorer's window: copy it into this one. */
function own(identity: AuthIdentity | null): AuthIdentity | null {
  return identity ? (JSON.parse(JSON.stringify(identity)) as AuthIdentity) : null;
}

/** Call before hydrating, so the first client render already signs as the Explorer account. */
export function adoptExplorerHost(win: Window): CreatorHubHost | null {
  const host = creatorHubHost(win);
  if (!host) return null;
  setHostIdentity(own(host.identity()));
  installHostSignIn(() => host.signIn());
  installHostWalletProvider(() => host.walletProvider());
  adopted = host;
  return host;
}

export function explorerHost(): CreatorHubHost | null {
  return adopted;
}

let accountReloads = 0;

/** Whether the hub is reloading its data for an Explorer account switch, which root's loader joins. */
export function reloadingForAccount(): boolean {
  return accountReloads > 0;
}

/**
 * Follows the Explorer's account switches: the session changes in place and `onChange` reloads the
 * page's data, counting as an account reload until what it returns settles. The subscription ends
 * when this document unloads, before the Explorer calls into it.
 */
export function followExplorerIdentity(win: Window, onChange: () => void | Promise<void>): () => void {
  if (!adopted) return () => {};
  const off = adopted.subscribe((identity) => {
    setHostIdentity(own(identity));
    accountReloads++;
    let reloading: void | Promise<void> = undefined;
    try {
      reloading = onChange();
    } finally {
      void Promise.resolve(reloading).finally(() => {
        accountReloads--;
      });
    }
  });
  win.addEventListener("pagehide", off);
  return () => {
    win.removeEventListener("pagehide", off);
    off();
  };
}

/**
 * The framed hub's side of the contract: links leaving /create go to the Explorer, Escape stays
 * the hub's, the Explorer's account switches reload the page's data (`reload`), and the wallet
 * cookie is taken back for the Explorer account whenever the hub is used again, since a website
 * tab used meanwhile points it at the website's own account.
 */
export function installHubFrame(win: Window, host: CreatorHubHost, reload: () => void | Promise<void>): () => void {
  const offGuards = installEmbedGuards(win, { scope: CREATOR_HUB_SCOPE, open: (url) => host.openLink(url) });
  const offIdentity = followExplorerIdentity(win, reload);
  const offClaim = claimWalletCookieOnUse(win);
  return () => {
    offGuards();
    offIdentity();
    offClaim();
  };
}

type HubLocation = { pathname: string; search: string; hash: string; key: string };

/**
 * Keeps a framed hub on its own pages. A location outside /create goes to the Explorer once (a
 * jump in, or a new tab), and the returned path is where the hub steps back to: the last hub page
 * it showed. Null means stay.
 */
export function hubLocationKeeper(host: CreatorHubHost): (location: HubLocation, base: string) => string | null {
  let last: string | null = null;
  let handled: string | null = null;
  return (location, base) => {
    const href = embedEscapeHref(location.pathname, location.search, location.hash, CREATOR_HUB_SCOPE);
    if (!href) {
      last = location.pathname + location.search + location.hash;
      return null;
    }
    if (handled === location.key) return null;
    handled = location.key;
    host.openLink(new URL(href, base).href);
    return last ?? CREATOR_HUB_HOME;
  };
}

/** A link the hub opens in a new tab; framed by the Explorer, the Explorer takes it (its /play links jump in). */
export function openHubLink(url: string): void {
  if (adopted) adopted.openLink(new URL(url, window.location.href).href);
  else window.open(url, "_blank", "noopener,noreferrer");
}

/**
 * The account menu's actions. Framed by the Explorer the account is the Explorer's: switching
 * opens its sign-in, and signing out is left to the Explorer's own menu.
 */
export function hubAccountActions(signOut: () => void): { onSignOut?: () => void; onSwitchAccount?: () => void } {
  const host = adopted;
  return host ? { onSwitchAccount: () => host.signIn() } : { onSignOut: signOut };
}
