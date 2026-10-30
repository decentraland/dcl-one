import Page from "@ui/shop/pages/OutfitStudio";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.outfits.manage";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("outfitStudioManage", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("outfitStudioManage");
export const headers = shopHeaders;

export default function ShopOutfitStudioManageRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
