import Page from "@ui/shop/pages/TokenRent";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.token.$contractAddress.$tokenId_.rent";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("tokenRent", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("tokenRent");
export const headers = shopHeaders;

export default function ShopTokenRentRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
