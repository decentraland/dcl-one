import { redirect } from "react-router";

import Page from "@ui/shop/pages/Credits";

import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";

import type { Route } from "./+types/shop.credits";

// The iOS app's web view sells credits through In-App Purchase, not here (upstream IapGate).
function iapRedirect(url: URL): string | null {
  return url.searchParams.get("view") === "mobile-iap" ? `/shop/overview${url.search}` : null;
}

export async function loader({ request, params }: Route.LoaderArgs) {
  const iap = iapRedirect(new URL(request.url));
  if (iap) throw redirect(iap);
  return shopLoader("credits", request, params);
}

export async function clientLoader({ request }: Route.ClientLoaderArgs) {
  const iap = iapRedirect(new URL(request.url));
  if (iap) throw redirect(iap);
  return shopClientLoader();
}

export const meta = shopMeta("credits");
export const headers = shopHeaders;

export default function ShopCreditsRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
