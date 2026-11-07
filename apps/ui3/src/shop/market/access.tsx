// Who sees the Marketplace inside the Shop: resales, bids, LAND, NAME resale, listing and selling, an item's owners
// and sales, favorites lists. Only a web3 wallet session (MetaMask and the like) while the `shop-marketplace-features`
// rollout is on, and only on a node that offers resales at all (the host's `secondarySales` capability). A web2
// (managed) session never sees them: those features pay in MANA and need a wallet the buyer holds. This only decides
// what is drawn: the APIs behind it are public, and the lock is the node's shop service refusing those calls under the
// same flag, so the flag is turned on only where the node does.
import type { ComponentType, ReactNode } from "react";
import { Navigate } from "react-router";

import { hostOr } from "../discovery/flags";
import { REMOTE_FLAG, useRemoteFlagState } from "../discovery/lib/remoteFlags";
import { shopPath } from "../routes";
import { useShopCapabilities } from "../seller/lib/featureFlags";
import { useShopWallet } from "../wallet/context";
import { marketplaceAccess, type MarketplaceAccess } from "./access-rule";

export { marketplaceAccess, needsMarketplace, type MarketplaceAccess } from "./access-rule";

export function useMarketplaceAccess(): MarketplaceAccess {
  // A host (the native shop, a journey) may declare the switch itself; the web3 session is still required.
  const flag = hostOr("marketplaceFeatures", useRemoteFlagState(REMOTE_FLAG.marketplaceFeatures));
  const wallet = useShopWallet();
  const caps = useShopCapabilities();
  return marketplaceAccess(flag, wallet, caps.secondarySales);
}

/** Whether to draw a Marketplace feature now. Fails closed while the answer is pending. */
export function useMarketplace(): boolean {
  return useMarketplaceAccess() === "on";
}

/** A Marketplace page: nothing while pending, the Shop's overview when it is off for this visitor. */
export function MarketplaceGate({ children }: { children: ReactNode }) {
  const access = useMarketplaceAccess();
  if (access === "pending") return null;
  if (access === "off") return <Navigate to={shopPath("overview")} replace />;
  return <>{children}</>;
}

export function marketplacePage<P extends object>(Page: ComponentType<P>): ComponentType<P> {
  const Gated = (props: P) => (
    <MarketplaceGate>
      <Page {...props} />
    </MarketplaceGate>
  );
  Gated.displayName = `Marketplace(${Page.displayName ?? Page.name ?? "Page"})`;
  return Gated;
}
