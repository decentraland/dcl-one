import Page from "@ui/shop/pages/Activity";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.activity";

export const loader = ({ request, params, context }: Route.LoaderArgs) => shopLoader("activity", request, params, context);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("activity");
export const headers = shopHeaders;

export default function ShopActivityRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
