import Page from "@ui/shop/pages/Items";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.items";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("items", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("items");
export const headers = shopHeaders;

export default function ShopItemsRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
