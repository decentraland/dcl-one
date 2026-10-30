import { redirect } from "react-router";

import { shopIndexTarget } from "@/core/lib/shop/redirects";

import type { Route } from "./+types/shop._index";

// /shop is the overview (the upstream Shop's `/` alias); v2's /shop?tab= opens the page that tab became.
export function loader({ request }: Route.LoaderArgs) {
  const url = new URL(request.url);
  return redirect(shopIndexTarget(url.search), 308);
}

export function clientLoader({ request }: Route.ClientLoaderArgs) {
  const url = new URL(request.url);
  return redirect(shopIndexTarget(url.search, url.hash));
}

export default function ShopIndex() {
  return null;
}
