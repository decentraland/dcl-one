import Page from "@ui/shop/pages/OutfitDetail";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.items_.outfits.$id";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("outfitDetail", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("outfitDetail");
export const headers = shopHeaders;

export default function ShopOutfitRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
