import { useEffect, useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createRoot } from "react-dom/client";
import { createMemoryRouter, RouterProvider } from "react-router";

import type { ShopHost, ShopMount } from "@ui/shop/native-contract";
import { isShopPath } from "@ui/shop/native-contract";

import { setHostIdentity } from "@/data/lib/auth/session";
import { installHostWalletProvider } from "@/data/lib/auth/wallet";
import { installHostSignIn } from "@/features/components/auth/signin-store";

import { primeDappsFlags } from "@/data/lib/feature-flags.browser";

import { installSitesShopClient } from "./host";
import { nativeShopRoutes } from "./native-routes";
import "./native.css";

// The Explorer's native Shop (ui3/src/shop/native-contract.ts, version 1): the same ui3 Shop pages and
// route table the website serves, in the Explorer's own document and memory router, on the identity and
// wallet the Explorer hands over. Old /marketplace paths resolve to their Shop page here too.

export function mountShop(container: HTMLElement, host: ShopHost): ShopMount {
  let closed = false;
  setHostIdentity(host.identity);
  installSitesShopClient();
  const uninstallSignIn = installHostSignIn(host.onSignIn);
  const uninstallWallet = host.walletProvider
    ? installHostWalletProvider(host.walletProvider)
    : undefined;
  const router = createMemoryRouter(nativeShopRoutes(host), {
    initialEntries: [isShopPath(host.path) ? host.path : "/shop"],
  });
  let previous = host.path;
  const unsubscribe = router.subscribe((state) => {
    if (state.navigation.state !== "idle") return;
    const path =
      state.location.pathname + state.location.search + state.location.hash;
    if (path !== previous) {
      previous = path;
      host.onNavigate(path);
    }
  });
  const onClick = (event: MouseEvent) => {
    if (
      event.defaultPrevented ||
      event.button !== 0 ||
      event.ctrlKey ||
      event.metaKey ||
      event.shiftKey ||
      event.altKey
    )
      return;
    const anchor =
      event.target instanceof Element
        ? event.target.closest<HTMLAnchorElement>("a[href]")
        : null;
    if (!anchor || anchor.hasAttribute("download")) return;
    const url = new URL(anchor.href, location.href);
    if (!/^https?:$/.test(url.protocol)) return;
    event.preventDefault();
    const path = url.pathname + url.search + url.hash;
    if (url.origin === location.origin && isShopPath(path))
      void router.navigate(path);
    else host.onExternalLink(url.href);
  };
  container.addEventListener("click", onClick);
  container.classList.add("mk-native");
  const root = createRoot(container);
  function NativeRoot() {
    const [queryClient] = useState(
      () =>
        new QueryClient({
          defaultOptions: { queries: { retry: false, staleTime: 30_000 } },
        }),
    );
    // Styles and native DOM share the Explorer document; the router owns only this subtree.
    useEffect(() => {
      container.dataset.marketplaceReady = "true";
      container.dataset.shopReady = "true";
      return () => {
        delete container.dataset.marketplaceReady;
        delete container.dataset.shopReady;
        void queryClient.cancelQueries();
        queryClient.clear();
      };
    }, [queryClient]);
    return (
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    );
  }
  // The first render waits (briefly) for the flags, so what the pages read and show agrees with them from the start.
  void primeDappsFlags().then(() => {
    if (!closed) root.render(<NativeRoot />);
  });
  return {
    updateIdentity(next) {
      if (closed) return;
      // An account switch is an event, not a reload (the Explorer keeps its engine): the cart and every
      // open purchase rescope to the new account through the wallet seam.
      setHostIdentity(next);
    },
    navigate(path) {
      if (!closed && isShopPath(path) && path !== previous)
        void router.navigate(path);
    },
    unmount() {
      if (closed) return;
      closed = true;
      unsubscribe();
      router.dispose();
      container.removeEventListener("click", onClick);
      root.unmount();
      container.classList.remove("mk-native");
      uninstallSignIn();
      uninstallWallet?.();
      setHostIdentity(undefined);
    },
  };
}

/** The old name the Explorer's marketplace runtime loader imports. */
export const mountMarketplace = mountShop;
