// The /shop route table picks this page up by file name (routes.tsx); it lives with its area in market/pages/FavoriteList.
import { marketplacePage } from "../market/access";
import Page from "../market/pages/FavoriteList";

// Favorites lists are the Marketplace's (market/access.tsx): only with the marketplace features on, for a web3 session.
export default marketplacePage(Page);
