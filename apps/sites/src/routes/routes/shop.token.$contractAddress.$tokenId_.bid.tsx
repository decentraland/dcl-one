import Page from "@ui/shop/pages/TokenBid";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.token.$contractAddress.$tokenId_.bid";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("tokenBid", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("tokenBid");
export const headers = shopHeaders;

export default function ShopTokenBidRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
