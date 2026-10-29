import Page from "@ui/shop/pages/Event";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.event";

export const loader = ({ request, params }: Route.LoaderArgs) => shopLoader("event", request, params);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("event");
export const headers = shopHeaders;

export default function ShopEventRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
