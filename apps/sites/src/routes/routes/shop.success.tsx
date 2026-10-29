import Page from "@ui/shop/pages/Success";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.success";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("success", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("success");
export const headers = shopHeaders;

export default function ShopSuccessRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
