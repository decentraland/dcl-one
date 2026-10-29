import Page from "@ui/shop/pages/TokenBid";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.item.$contractAddress.$itemId_.bid";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("itemBid", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("itemBid");
export const headers = shopHeaders;

export default function ShopItemBidRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
