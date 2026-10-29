import { useMemo } from "react";
import { Outlet, redirect, useRevalidator, useRouteError, type LoaderFunctionArgs, type RouteObject } from "react-router";

import ShopChrome from "@ui/shop/discovery/frames/ShopChrome";
import type { ShopHost } from "@ui/shop/native-contract";
import { NativeShopContext } from "@ui/shop/native-context";
import { shopRouteObjects } from "@ui/shop/routes";
import type { ShopWalletHost } from "@ui/shop/wallet/session";
import type { AuthIdentity } from "@ui/data/auth/identity";

import { legacyShopTarget } from "@/core/lib/shop/redirects";
import { useAuth } from "@/data/lib/auth/context";
import ChromeNavBridge from "@/features/components/chrome/ChromeNavBridge";

import { ShopProviders } from "./host";

// The native Shop's route table (native.client.tsx mounts it): ui3's /shop route table inside the Shop
// frame, on the identity and wallet the Explorer hands over, with /shop's index and the old
// /marketplace paths resolved by the website's own redirect table before anything renders.

function Failure() {
  const error = useRouteError();
  const retry = useRevalidator();
  return (
    <section className="mk-native__status" role="alert">
      <h2>Couldn't open the Shop</h2>
      <p>{error instanceof Error ? error.message : "This screen is unavailable. Please try again."}</p>
      <button type="button" onClick={() => void retry.revalidate()}>
        Try again
      </button>
    </section>
  );
}

/** /shop itself (v2's ?tab=) and the old /marketplace paths, resolved before anything renders. */
function legacyRedirect({ request }: LoaderFunctionArgs) {
  const url = new URL(request.url);
  return redirect(legacyShopTarget(url.pathname, url.search, url.hash) ?? "/shop/overview");
}

function nativeWalletHost(host: ShopHost, identity: AuthIdentity | null): ShopWalletHost {
  return {
    identity,
    // The Explorer declares its wallet: a provider it hands over is a web3 wallet the buyer signs with,
    // unless it says the provider is its managed (in-app) wallet's signer.
    walletKind: identity ? (host.walletKind ?? (host.walletProvider ? "web3" : "managed")) : null,
    provider: host.walletProvider ? () => host.walletProvider!() : null,
    onSignIn: () => host.onSignIn(),
    onConnectWallet: () => host.onSignIn(),
    onSwitchAccount: () => host.onSignIn(),
  };
}

function NativeLayout({ host }: { host: ShopHost }) {
  const { identity } = useAuth();
  const walletHost = useMemo(() => nativeWalletHost(host, identity as AuthIdentity | null), [host, identity]);
  // Stripe and card providers open in the Explorer's own browser, never in place of the engine.
  const services = useMemo(() => ({ openExternal: (url: string) => host.onExternalLink(url) }), [host]);
  return (
    <NativeShopContext.Provider value={true}>
      <ChromeNavBridge>
        <ShopProviders host={walletHost} services={services}>
          <ShopChrome>
            <div className="mk-native__view">
              <Outlet />
            </div>
          </ShopChrome>
        </ShopProviders>
      </ChromeNavBridge>
    </NativeShopContext.Provider>
  );
}

/** The routes the native Shop serves: /shop (its index resolving v2's ?tab=) and the old /marketplace paths. */
export function nativeShopRoutes(host: ShopHost): RouteObject[] {
  const pages = shopRouteObjects().filter((route) => !route.index);
  return [
    {
      id: "root",
      element: <NativeLayout host={host} />,
      ErrorBoundary: Failure,
      children: [
        { path: "/shop", children: [{ index: true, loader: legacyRedirect }, ...pages] },
        { path: "/marketplace", loader: legacyRedirect },
        { path: "/marketplace/*", loader: legacyRedirect },
        { path: "*", loader: () => redirect("/shop/overview") },
      ],
    },
  ];
}
