// from: shop@94584a5c:app/src/lib/marketplace.ts
// The off-chain marketplace address book: where a new listing goes (the newest version deployed on a
// chain), and which version a trade can settle on (the one it was signed for, on its own chain; any
// other address is a contract that never signed it). Each version redeems coupons signed against its
// own manager only.

import type { ChainId } from "../../model/chain";
import { chainContracts, type OffChainMarketplace } from "../chain/contracts";

/** The newest off-chain marketplace deployed on `chainId`; throws where none is. */
export function getLatestOffChainMarketplaceContract(chainId: ChainId): OffChainMarketplace {
  const first = chainContracts(chainId)?.offChainMarketplaces[0];
  if (!first) throw new Error(`No off-chain marketplace contract exists on chain ${chainId}`);
  return first;
}

/** The marketplace a trade names, or null when that address is not a marketplace version on the trade's chain. */
export function getMarketplaceForTrade(trade: { contract: string; chainId: number }): OffChainMarketplace | null {
  const address = trade.contract?.toLowerCase();
  return chainContracts(trade.chainId)?.offChainMarketplaces.find((m) => m.address === address) ?? null;
}

/** The coupon manager of the marketplace a trade names, lowercase, or null. */
export function getCouponManagerForTrade(trade: { contract: string; chainId: number }): string | null {
  return getMarketplaceForTrade(trade)?.couponManager ?? null;
}
