import Page from "@ui/shop/pages/FavoriteList";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.my-favorites_.$listId";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("favoriteList", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("favoriteList");
export const headers = shopHeaders;

export default function ShopFavoriteListRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
