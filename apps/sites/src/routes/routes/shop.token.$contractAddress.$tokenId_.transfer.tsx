import Page from "@ui/shop/pages/TokenTransfer";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.token.$contractAddress.$tokenId_.transfer";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("tokenTransfer", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("tokenTransfer");
export const headers = shopHeaders;

export default function ShopTokenTransferRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
