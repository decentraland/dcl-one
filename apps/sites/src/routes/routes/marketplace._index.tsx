import { redirect } from "react-router";

import { marketplaceTarget } from "@/core/lib/shop/redirects";

import type { Route } from "./+types/marketplace._index";

// The Marketplace became the Shop (SHOP.md): /marketplace is the Shop's overview, or its items in the
// iOS app's IAP view, query kept. GET only: this route has no action, so nothing is ever re-posted.
export function loader({ request }: Route.LoaderArgs) {
  const url = new URL(request.url);
  return redirect(marketplaceTarget("/marketplace", url.search) ?? "/shop/overview", 308);
}

export function clientLoader({ request }: Route.ClientLoaderArgs) {
  const url = new URL(request.url);
  return redirect(marketplaceTarget("/marketplace", url.search, url.hash) ?? "/shop/overview");
}

export default function MarketplaceIndexRedirect() {
  return null;
}
