import Page from "@ui/shop/pages/MyStore";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.my-store";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("myStore", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("myStore");
export const headers = shopHeaders;

export default function ShopMyStoreRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
