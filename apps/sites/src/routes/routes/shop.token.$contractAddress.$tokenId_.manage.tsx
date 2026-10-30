import Page from "@ui/shop/pages/TokenManage";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.token.$contractAddress.$tokenId_.manage";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("tokenManage", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("tokenManage");
export const headers = shopHeaders;

export default function ShopTokenManageRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
