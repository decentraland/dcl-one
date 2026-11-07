// /shop/settings (routes.tsx picks this file up by name; /shop/authorizations aliases here). The page lives in ../checkout.
import { marketplacePage } from "../market/access";
import Page from "../checkout/SettingsPage";

// A Marketplace page: only with the marketplace features on, for a web3 wallet session (market/access.tsx).
export default marketplacePage(Page);
