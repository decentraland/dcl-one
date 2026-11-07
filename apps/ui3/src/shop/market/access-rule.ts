// The Marketplace's rule, without React: who has it (market/access.tsx) and which cart lines are its. Kept apart so a
// host's own pages (sites) can apply it without the Shop's components.
import { assetKindOf } from "../discovery/cardPrice";
import type { CartLine } from "../model/purchase";
import type { Session } from "../model/settlement";

/** `pending` while the flag has not answered or the wallet session is still being read back: show neither. */
export type MarketplaceAccess = "pending" | "on" | "off";

export function marketplaceAccess(flag: boolean | null, wallet: { session: Session; resolving: boolean }, offered: boolean): MarketplaceAccess {
  if (!offered || flag === false) return "off";
  if (flag === null || wallet.resolving) return "pending";
  return wallet.session.kind === "web3" ? "on" : "off";
}

/** A cart line the Marketplace sells: a resale (an existing token, LAND and NAME resale included) or a rental. A
 * collection item's mint and a NAME registration are the Shop's own. */
export function needsMarketplace(line: CartLine): boolean {
  if (line.listing.rentalId) return true;
  if (line.asset.token.kind === "item") return false;
  return !(assetKindOf(line.display.category ?? "") === "name" && !!line.listing.name);
}
