// The /shop route table picks this page up by file name (routes.tsx); it lives with its area in seller/components/RemoveListingModal.
import { marketplacePage } from "../market/access";
import Page from "../seller/components/RemoveListingModal";

// A Marketplace page: only with the marketplace features on, for a web3 wallet session (market/access.tsx).
export default marketplacePage(Page);
