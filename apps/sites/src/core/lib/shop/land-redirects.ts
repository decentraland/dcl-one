import { landNetwork, selectLandChain } from "@shop/land/contracts";
import { landIdentity as resolveLandIdentity, landRedirectTarget as resolveLandRedirect, type LandRedirect } from "@shop/land/redirects";
import { shopChainEnvironment } from "@web-core/shop-chain-env";

export function landIdentity(query: URLSearchParams) {
  return resolveLandIdentity(query, selectLandChain(shopChainEnvironment()));
}

export function landRedirectTarget(kind: LandRedirect, search: string): string {
  return resolveLandRedirect(kind, search, selectLandChain(shopChainEnvironment()));
}

/** The LAND and Estate contracts of this deployment's network, so a /shop alias names the same chain as a LAND redirect. */
export function landAliasOptions(): { contracts: { land: string; estate: string } } {
  const { land, estate } = landNetwork(selectLandChain(shopChainEnvironment()));
  return { contracts: { land, estate } };
}
