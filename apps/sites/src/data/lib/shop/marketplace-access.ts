// Whether this visitor has the Shop's Marketplace, for sites' own pages that link into it (the Creator Hub's LAND
// and NAME links): the rule the Shop's pages apply (ui3 market/access.tsx), with the session kind this site's
// sign-in recorded (data/lib/auth/shop-wallet.ts). Off, a link would only bounce to the Shop's overview.
import { useEffect, useSyncExternalStore } from "react";

import { hostOr } from "@ui/shop/discovery/flags";
import { REMOTE_FLAG, useRemoteFlagState } from "@ui/shop/discovery/lib/remoteFlags";
import { marketplaceAccess, marketplaceSession } from "@shop/market/access-rule";
import { SIGNED_OUT } from "@shop/model/settlement";
import { useShopCapabilities } from "@ui/shop/seller/lib/featureFlags";
import { shopSession } from "@ui/shop/wallet/session";

import { useAuth } from "../auth/index";
import { primeDappsFlags } from "../feature-flags.browser";
import { currentWalletKind, web3Usable } from "../auth/shop-wallet";

const noSubscription = () => () => {};

export function useMarketplaceHere(): boolean {
  const { identity } = useAuth();
  // These pages are not the Shop's, so nothing primed the flags: the service would tell this site's own Referer that
  // every flag is off, so they come from this server's reading, as a client navigation into the Shop does.
  useEffect(() => {
    void primeDappsFlags();
  }, []);
  const flag = hostOr("marketplaceFeatures", useRemoteFlagState(REMOTE_FLAG.marketplaceFeatures));
  const caps = useShopCapabilities();
  // The session is this browser's: unknown on the server and until hydration, so nothing is drawn before it.
  const hydrated = useSyncExternalStore(noSubscription, () => true, () => false);
  const kind = hydrated && identity ? currentWalletKind(identity) : null;
  // The Shop's own session rule (an expired identity is signed out), then its Marketplace rule: a guessed web3 is web2.
  const session = kind && identity ? marketplaceSession(shopSession({ identity, walletKind: kind }), web3Usable(identity)) : SIGNED_OUT;
  return marketplaceAccess(flag, { session, resolving: !hydrated }, caps.secondarySales) === "on";
}
