// The Shop's host in sites: ONE wallet session (sites' signed identity), ONE cart and ONE /shop-api
// client for every /shop page, around ui3's Shop frame. The pages themselves are ui3/src/shop's; this
// file only says who is signed in, how (a web3 wallet or a managed in-app wallet, a fact of the sign-in,
// never inferred from an address or a balance), what signs for it (the browser wallet, or the in-app
// signer for a managed session, so a web2 buyer can confirm the credits purchase that is their main
// action), how to sign in, and where the node is.

import { useMemo, useRef, useState, useSyncExternalStore, type ReactNode } from "react";
import { useLocation } from "react-router";

import { createShopClient, identitySigner, setShopClient } from "@ui/shop/api";
import { ShopServicesProvider, type ShopServices } from "@ui/shop/checkout/services";
import ShopChrome from "@ui/shop/discovery/frames/ShopChrome";
import { ShopWalletProvider } from "@ui/shop/wallet/context";
import type { ShopWalletHost } from "@ui/shop/wallet/session";
import type { AuthIdentity } from "@ui/data/auth/identity";

import { useAuth } from "@/data/lib/auth/index";
import { getIdentity } from "@/data/lib/auth/session";
import { browserProvider, currentWalletKind, inAppSessionFor, managedShopProvider } from "@/data/lib/auth/shop-wallet";
import { openSignIn } from "@/features/components/auth/signin-store";

let installed = false;

/** The document's Shop client signs with sites' current identity, read per request. */
export function installSitesShopClient(): void {
  if (installed || typeof window === "undefined") return;
  installed = true;
  setShopClient(createShopClient({ signer: () => identitySigner(getIdentity() as AuthIdentity | null) }));
}

const noSubscription = () => () => {};

/** The wallet host for the Shop from sites' auth; the return destination is the page the buyer is on. */
export function useSitesShopHost(): ShopWalletHost {
  const auth = useAuth();
  // The session lives in localStorage, which the server and the hydrating render cannot read: until
  // hydration is over, a null identity is unknown rather than signed out.
  const hydrated = useSyncExternalStore(noSubscription, () => true, () => false);
  const location = useLocation();
  const identity = auth.identity as AuthIdentity | null;
  const returnTo = useRef("");
  returnTo.current = `${location.pathname}${location.search}${location.hash}`;
  const { disconnect } = auth;
  return useMemo<ShopWalletHost>(() => {
    const walletKind = typeof window === "undefined" ? null : currentWalletKind(identity);
    const signIn = () => openSignIn({ redirectTo: returnTo.current });
    return {
      identity,
      walletKind,
      provider:
        walletKind === "web3"
          ? browserProvider
          : identity && inAppSessionFor(identity.signer)
            ? () => managedShopProvider(identity.signer)
            : null,
      onSignIn: signIn,
      onConnectWallet: signIn,
      // Signing in again leaves the page: Magic's logout gets to finish first.
      onSwitchAccount: () => {
        void disconnect().then(signIn);
      },
      resolving: !hydrated,
    };
  }, [identity, disconnect, hydrated]);
}

/** The providers every Shop page needs, for a host that already knows its wallet host. */
export function ShopProviders({ host, services, children }: { host: ShopWalletHost; services?: Partial<ShopServices>; children: ReactNode }) {
  const value = useMemo<Partial<ShopServices>>(
    () => ({ signer: identitySigner(host.identity), ...services }),
    [host.identity, services],
  );
  return (
    <ShopWalletProvider host={host}>
      <ShopServicesProvider value={value}>{children}</ShopServicesProvider>
    </ShopWalletProvider>
  );
}

/** The /shop layout: sites' session around ui3's Shop frame. */
export default function SitesShopFrame({ children }: { children: ReactNode }) {
  useState(installSitesShopClient);
  const host = useSitesShopHost();
  return (
    <ShopProviders host={host}>
      <ShopChrome>{children}</ShopChrome>
    </ShopProviders>
  );
}
