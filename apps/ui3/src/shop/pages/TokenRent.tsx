// /shop/token/:contractAddress/:tokenId/rent (v2 /marketplace/rent-land lands here): rent a LAND or an estate, in MANA.
import { marketplacePage } from "../market/access";
import Page from "../land/RentPage";

// A Marketplace page: only with the marketplace features on, for a web3 wallet session (market/access.tsx).
export default marketplacePage(Page);
