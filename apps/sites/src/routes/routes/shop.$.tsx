import { redirect } from "react-router";

import NotFound from "@ui/shop/pages/NotFound";
import { resolveShopAlias } from "@ui/shop/routes";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.$";

// Every other /shop path: the upstream Shop's aliases (market, my-purchases, import, authorizations,
// assets/*, my-assets) and the retained Marketplace resolvers (browse, parcels/:x/:y, estates/:id)
// redirect keeping identity, query and hash; anything else is the Shop's not-found page, with a 404.
export async function loader({ request, params }: Route.LoaderArgs) {
  const url = new URL(request.url);
  const target = resolveShopAlias(url.pathname, url.search);
  if (target) throw redirect(target, 308);
  return shopLoader("notFound", request, params);
}

export function clientLoader({ request }: Route.ClientLoaderArgs) {
  const url = new URL(request.url);
  const target = resolveShopAlias(url.pathname, url.search, url.hash);
  if (target) throw redirect(target);
  return null;
}

export const meta = shopMeta("notFound");
export const headers = shopHeaders;

export default function ShopNotFoundRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <NotFound />
    </ShopPage>
  );
}
