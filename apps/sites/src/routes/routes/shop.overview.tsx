import { prefetchOverviewRails } from "@ui/shop/discovery/pages/overviewRails";
import Page from "@ui/shop/pages/Overview";

import { installSitesShopClient } from "@/data/lib/auth/shop-client";
import { primeDappsFlags } from "@/data/lib/feature-flags.browser";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.overview";

export const loader = ({ request, params, context }: Route.LoaderArgs) => shopLoader("overview", request, params, context);
/** shopClientLoader, with the rails sent beside the flags read: a link to the overview shows them without a mount's wait. */
export async function clientLoader(): Promise<null> {
  const flags = primeDappsFlags();
  installSitesShopClient();
  prefetchOverviewRails(flags);
  await flags;
  return null;
}
export const meta = shopMeta("overview");
export const headers = shopHeaders;

export default function ShopOverviewRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
