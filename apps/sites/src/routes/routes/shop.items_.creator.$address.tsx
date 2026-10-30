import Page from "@ui/shop/pages/Creator";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.items_.creator.$address";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("creator", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("creator");
export const headers = shopHeaders;

export default function ShopCreatorRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
