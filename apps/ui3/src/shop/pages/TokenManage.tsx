// /shop/token/:contractAddress/:tokenId/manage (marketplace@f3956826 /contracts/:contractAddress/tokens/:tokenId/manage;
// v2 /marketplace/manage and /builder/land-{operator,assign-ens} land here): an owner's LAND, estate or NAME.
import { marketplacePage } from "../market/access";
import Page from "../land/ManageAsset";

// A Marketplace page: only with the marketplace features on, for a web3 wallet session (market/access.tsx).
export default marketplacePage(Page);
