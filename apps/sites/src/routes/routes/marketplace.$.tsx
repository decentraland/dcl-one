import { redirect } from "react-router";

import { marketplaceTarget } from "@/core/lib/shop/redirects";

import type { Route } from "./+types/marketplace.$";

// Every old Marketplace GET page, v2's /marketplace/* and the classic Marketplace's paths alike, lands
// on the Shop page that does its job (core/lib/shop/redirects.ts), keeping the asset's identity, the
// query and the return destination. GET only: this route has no action, so a write is never redirected.
export function loader({ request }: Route.LoaderArgs) {
  const url = new URL(request.url);
  return redirect(marketplaceTarget(url.pathname, url.search) ?? "/shop/overview", 308);
}

export function clientLoader({ request }: Route.ClientLoaderArgs) {
  const url = new URL(request.url);
  return redirect(marketplaceTarget(url.pathname, url.search, url.hash) ?? "/shop/overview");
}

export default function MarketplaceRedirect() {
  return null;
}
