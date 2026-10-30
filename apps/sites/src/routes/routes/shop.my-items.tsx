import Page from "@ui/shop/pages/MyItems";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.my-items";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("myItems", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("myItems");
export const headers = shopHeaders;

export default function ShopMyItemsRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
