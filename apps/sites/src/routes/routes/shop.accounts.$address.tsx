import Page from "@ui/shop/pages/Account";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.accounts.$address";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("account", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("account");
export const headers = shopHeaders;

export default function ShopAccountRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
