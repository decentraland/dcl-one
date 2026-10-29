import Page from "@ui/shop/pages/TokenSell";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.token.$contractAddress.$tokenId_.sell";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("tokenSell", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("tokenSell");
export const headers = shopHeaders;

export default function ShopTokenSellRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
