// /shop/settings (routes.tsx picks this file up by name; /shop/authorizations aliases here). The page lives in ../checkout.
// Open to every session: it also lists the Shop's own approvals. The Marketplace's are listed only with it on.
export { default } from "../checkout/SettingsPage";
