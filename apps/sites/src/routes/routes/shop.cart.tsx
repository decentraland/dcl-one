import Page from "@ui/shop/pages/Cart";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.cart";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("cart", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("cart");
export const headers = shopHeaders;

export default function ShopCartRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
