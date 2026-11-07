// Who sees the Marketplace inside the Shop: resales, bids, LAND, NAME resale, listing and selling, an item's owners
// and sales, favorites lists. Only a web3 wallet session (MetaMask and the like) while the `shop-marketplace-features`
// rollout is on, and only on a node that offers resales at all (the host's `secondarySales` capability). A web2
// (managed) session never sees them: those features pay in MANA and need a wallet the buyer holds. This only decides
// what is drawn: the APIs behind it are public, and the lock is the node's shop service refusing those calls under the
// same flag, so the flag is turned on only where the node does.
import type { ComponentType, ReactNode } from "react";
import { Navigate } from "react-router";

import { assetKindOf } from "../discovery/cardPrice";
import { hostShopFlags } from "../discovery/flags";
import { REMOTE_FLAG, useRemoteFlagState } from "../discovery/lib/remoteFlags";
import type { CartLine } from "../model/purchase";
import type { Session } from "../model/settlement";
import { shopPath } from "../routes";
import { useShopCapabilities } from "../seller/lib/featureFlags";
import { useShopWallet } from "../wallet/context";

/** `pending` while the flag has not answered or the wallet session is still being read back: show neither. */
export type MarketplaceAccess = "pending" | "on" | "off";

export function marketplaceAccess(flag: boolean | null, wallet: { session: Session; resolving: boolean }, offered: boolean): MarketplaceAccess {
  if (!offered || flag === false) return "off";
  if (flag === null || wallet.resolving) return "pending";
  return wallet.session.kind === "web3" ? "on" : "off";
}

export function useMarketplaceAccess(): MarketplaceAccess {
  const remote = useRemoteFlagState(REMOTE_FLAG.marketplaceFeatures);
  // A host (the native shop, a journey) may declare the switch itself; the web3 session is still required.
  const declared = hostShopFlags().marketplaceFeatures;
  const flag = typeof declared === "boolean" ? declared : remote;
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

/** A cart line the Marketplace sells: a resale (an existing token, LAND and NAME resale included) or a rental. A
 * collection item's mint and a NAME registration are the Shop's own. */
export function needsMarketplace(line: CartLine): boolean {
  if (line.listing.rentalId) return true;
  if (line.asset.token.kind === "item") return false;
  return !(assetKindOf(line.display.category ?? "") === "name" && !!line.listing.name);
}
