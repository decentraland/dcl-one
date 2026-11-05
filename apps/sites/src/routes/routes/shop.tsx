import { Outlet } from "react-router";

import SitesShopFrame from "../shop/host";

// The Shop's Inter (shop@3a322cf9 app/index.html loads Inter v20 from Google Fonts in its head), preloaded so
// the first paint and every capture use Inter, not a system fallback, while the Shop's CSS is on its way.
// Only the Latin subset, one variable file for every weight: the other scripts load later, and only for text
// that needs them.
export const links = () => [
  { rel: "preload", as: "font", type: "font/woff2", crossOrigin: "anonymous" as const, href: "/fonts/Inter-v20-latin.woff2" },
];

// The /shop layout (ui3/src/shop/routes.tsx `SHOP_PAGES`): every child route renders a ui3 Shop
// page (ui3/src/shop/pages) inside the Shop frame, with sites' one wallet session, cart and client.
export default function ShopLayout() {
  return (
    <SitesShopFrame>
      <Outlet />
    </SitesShopFrame>
  );
}
