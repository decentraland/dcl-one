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
import type { ShopWalletHost, WalletKind } from "@ui/shop/wallet/session";
import type { AuthIdentity } from "@ui/data/auth/identity";

import { useAuth } from "@/data/lib/auth/index";
import { getIdentity } from "@/data/lib/auth/session";
import { getThirdwebSession, type ThirdwebSession } from "@/data/lib/auth/thirdweb/session";
import { signTypedData } from "@/data/lib/auth/typed-data";
import { hasWallet, walletProvider, WalletError, type Eip1193Provider } from "@/data/lib/auth/wallet";
import { openSignIn } from "@/features/components/auth/signin-store";

/**
 * How the signed identity was made: an in-app (thirdweb) session for the same account is a managed
 * wallet; otherwise the identity was signed by a browser wallet, which is web3 while one is present.
 */
export function sitesWalletKind(
  identity: Pick<AuthIdentity, "signer"> | null,
  facts: { inApp: ThirdwebSession | null; injected: boolean },
): WalletKind | null {
  if (!identity) return null;
  if (facts.inApp && facts.inApp.address.toLowerCase() === identity.signer.toLowerCase()) return "managed";
  return facts.injected ? "web3" : "managed";
}

function browserProvider(): Eip1193Provider | null {
  try {
    return walletProvider();
  } catch {
    return null;
  }
}

type TypedData = Parameters<typeof signTypedData>[0];

/**
 * The managed (in-app) wallet as the Shop's signing seam: it names its account and signs EIP-712 data
 * through the in-app signer, which is all the relayed rails (credits, approvals) ask of it. It holds
 * no gas and sends nothing itself, so every other request is refused; reads go over the Shop's RPC.
 */
export function managedShopProvider(account: string, sign: (typedData: TypedData, from: string) => Promise<string> = signTypedData): Eip1193Provider {
  const self = account.toLowerCase();
  return {
    async request({ method, params }) {
      switch (method) {
        case "eth_accounts":
        case "eth_requestAccounts":
          return [self];
        case "eth_signTypedData_v4": {
          const [from, data] = (params ?? []) as [string, unknown];
          if (typeof from !== "string" || from.toLowerCase() !== self) throw new WalletError("The in-app wallet signs only for its own account.", 4100);
          return sign((typeof data === "string" ? JSON.parse(data) : data) as TypedData, self);
        }
        default:
          throw new WalletError(`The in-app wallet does not support ${method}.`, 4200);
      }
    },
  };
}

let installed = false;

/** The document's Shop client signs with sites' current identity, read per request. */
export function installSitesShopClient(): void {
  if (installed || typeof window === "undefined") return;
  installed = true;
  setShopClient(createShopClient({ signer: () => identitySigner(getIdentity() as AuthIdentity | null) }));
}

function inAppFor(identity: Pick<AuthIdentity, "signer">): boolean {
  const inApp = getThirdwebSession();
  return !!inApp && inApp.address.toLowerCase() === identity.signer.toLowerCase();
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
    const walletKind =
      typeof window === "undefined" ? null : sitesWalletKind(identity, { inApp: getThirdwebSession(), injected: hasWallet() });
    const signIn = () => openSignIn({ redirectTo: returnTo.current });
    return {
      identity,
      walletKind,
      provider:
        walletKind === "web3"
          ? browserProvider
          : identity && inAppFor(identity)
            ? () => managedShopProvider(identity.signer)
            : null,
      onSignIn: signIn,
      onConnectWallet: signIn,
      onSwitchAccount: () => {
        disconnect();
        signIn();
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
