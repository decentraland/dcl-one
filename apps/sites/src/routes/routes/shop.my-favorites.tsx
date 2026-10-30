import Page from "@ui/shop/pages/MyFavorites";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.my-favorites";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("myFavorites", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("myFavorites");
export const headers = shopHeaders;

export default function ShopMyFavoritesRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
