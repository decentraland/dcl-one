import Page from "@ui/shop/pages/Names";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.names";

// No clientLoader: the server loader also answers client navigations here, for the telemetry session id.
export const loader = ({ request, params, context }: Route.LoaderArgs) => shopLoader("names", request, params, context);
export const meta = shopMeta("names");
export const headers = shopHeaders;

export default function ShopNamesRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
