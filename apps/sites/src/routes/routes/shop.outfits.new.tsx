import Page from "@ui/shop/pages/OutfitStudio";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.outfits.new";

export const loader = ({ request, params, context }: Route.LoaderArgs) => shopLoader("outfitStudioNew", request, params, context);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("outfitStudioNew");
export const headers = shopHeaders;

export default function ShopOutfitStudioNewRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
