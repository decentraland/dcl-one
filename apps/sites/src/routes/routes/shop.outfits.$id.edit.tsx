import Page from "@ui/shop/pages/OutfitStudio";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.outfits.$id.edit";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("outfitStudioEdit", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("outfitStudioEdit");
export const headers = shopHeaders;

export default function ShopOutfitStudioEditRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
