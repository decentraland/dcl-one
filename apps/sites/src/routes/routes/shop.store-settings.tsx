import Page from "@ui/shop/pages/StoreSettings";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.store-settings";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("storeSettings", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("storeSettings");
export const headers = shopHeaders;

export default function ShopStoreSettingsRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
