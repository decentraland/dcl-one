import Page from "@ui/shop/components/RemoveListingModal";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.token.$contractAddress.$tokenId_.cancel";

export const loader = ({ request, params, context }: Route.LoaderArgs) => shopLoader("tokenCancel", request, params, context);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("tokenCancel");
export const headers = shopHeaders;

export default function ShopTokenCancelRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
