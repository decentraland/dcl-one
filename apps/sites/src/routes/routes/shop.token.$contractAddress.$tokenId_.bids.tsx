import Page from "@ui/shop/workflows/AcceptBid";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.token.$contractAddress.$tokenId_.bids";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("tokenBids", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("tokenBids");
export const headers = shopHeaders;

export default function ShopTokenBidsRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
