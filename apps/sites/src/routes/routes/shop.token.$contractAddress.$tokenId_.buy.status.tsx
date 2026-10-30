import Page from "@ui/shop/pages/BuyStatus";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.token.$contractAddress.$tokenId_.buy.status";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("tokenBuyStatus", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("tokenBuyStatus");
export const headers = shopHeaders;

export default function ShopTokenBuyStatusRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
