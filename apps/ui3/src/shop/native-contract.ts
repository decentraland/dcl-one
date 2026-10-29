// from: v2 ui3/src/marketplace/native-contract.ts (moved here; the old path is removed)
// The versioned, same-document contract between the Explorer and the Shop runtime: no frame, cookies
// or second login session. The host supplies its active identity and wallet provider; the runtime
// navigates inside the Explorer's own route.

import type { AuthIdentity } from "../data/auth/identity";
import type { Eip1193Provider } from "../data/auth/wallet";

export type ShopHost = {
  identity: AuthIdentity | null;
  path: string;
  onNavigate(path: string): void;
  onSignIn(): void;
  onExternalLink(url: string): void;
  walletProvider?(): Eip1193Provider | null;
  /**
   * How the identity's account signs. Without it a provider means a web3 wallet. `managed`: the
   * provider is an in-app wallet's signer (EIP-712 only, no gas), for the relayed credits rails.
   */
  walletKind?: "web3" | "managed";
};

export type ShopMount = {
  updateIdentity(identity: AuthIdentity | null): void;
  navigate(path: string): void;
  unmount(): void;
};

export type ShopRuntime = {
  version: 1;
  mount(container: HTMLElement, host: ShopHost): ShopMount;
};

/** A path the Shop runtime renders: the canonical /shop pages and the old /marketplace ones it redirects. */
export function isShopPath(path: string): boolean {
  return /^\/(?:shop|marketplace)(?:[/?#]|$)/.test(path);
}

export type MarketplaceHost = ShopHost;
export type MarketplaceMount = ShopMount;
export type MarketplaceRuntime = ShopRuntime;
export const isMarketplacePath = isShopPath;
