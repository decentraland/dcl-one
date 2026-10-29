import Page from "@ui/shop/pages/Settings";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.settings";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("settings", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("settings");
export const headers = shopHeaders;

export default function ShopSettingsRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
