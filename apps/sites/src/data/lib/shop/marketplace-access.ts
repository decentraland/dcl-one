// Whether this visitor has the Shop's Marketplace, for sites' own pages that link into it (the Creator Hub's LAND
// and NAME links): the rule the Shop's pages apply (ui3 market/access.tsx), with the session kind this site's
// sign-in recorded (data/lib/auth/shop-wallet.ts). Off, a link would only bounce to the Shop's overview.
import { useSyncExternalStore } from "react";

import { hostOr } from "@ui/shop/discovery/flags";
import { REMOTE_FLAG, useRemoteFlagState } from "@ui/shop/discovery/lib/remoteFlags";
import { marketplaceAccess } from "@ui/shop/market/access-rule";
import { parseEvmAddress } from "@ui/shop/model/address";
import { SIGNED_OUT, type Session } from "@ui/shop/model/settlement";
import { useShopCapabilities } from "@ui/shop/seller/lib/featureFlags";

import { useAuth } from "../auth/index";
import { currentWalletKind } from "../auth/shop-wallet";

const noSubscription = () => () => {};

export function useMarketplaceHere(): boolean {
  const { identity } = useAuth();
  const flag = hostOr("marketplaceFeatures", useRemoteFlagState(REMOTE_FLAG.marketplaceFeatures));
  const caps = useShopCapabilities();
  // The session is this browser's: unknown on the server and until hydration, so nothing is drawn before it.
  const hydrated = useSyncExternalStore(noSubscription, () => true, () => false);
  const signer = identity ? parseEvmAddress(identity.signer) : null;
  const kind = hydrated && identity ? currentWalletKind(identity) : null;
  const session: Session = !signer || !kind ? SIGNED_OUT : kind === "web3" ? { kind: "web3", wallet: signer } : { kind: "web2", account: signer };
  return marketplaceAccess(flag, { session, resolving: !hydrated }, caps.secondarySales) === "on";
}
