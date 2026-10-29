import Page from "@ui/shop/pages/Lands";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.lands";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("lands", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("lands");
export const headers = shopHeaders;

export default function ShopLandsRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
