import Page from "@ui/shop/pages/Overview";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.overview";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("overview", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("overview");
export const headers = shopHeaders;

export default function ShopOverviewRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
