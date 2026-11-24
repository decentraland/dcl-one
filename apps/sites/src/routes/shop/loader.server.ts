import { data, type RouterContextProvider } from "react-router";
// The /shop route loaders: the page's SSR data (data/lib/shop/pages.server) with the session cookie
// every sites document keeps. Caching is set by the routes' `headers` (shopHeaders): private, except the
// cart document a shared cache may keep (shared-cart.ts).

import type { ShopPageId } from "@ui/shop/routes";

import { sidLoader } from "@/core/lib/telemetry/page-loader";
import { publicResponseContext } from "@/core/lib/telemetry/session";
import { track } from "@/core/lib/telemetry/track";
import { loadShopPage } from "@/data/lib/shop/pages.server";

export async function shopLoader(
  page: ShopPageId,
  request: Request,
  params: Record<string, string | undefined>,
  context: Readonly<RouterContextProvider>,
) {
  // A document a shared cache may keep carries no session id.
  if (context.get(publicResponseContext)) {
    const shared = await loadShopPage(page, { request, params });
    return data(shared.data, { status: shared.status });
  }
  const session = sidLoader(request, context);
  const load = await loadShopPage(page, { request, params });
  const url = new URL(request.url);
  // A friend-first-wear notification links the item page (src=ffw): its landing is that funnel's metric.
  if (page === "itemDetail" && url.searchParams.get("src") === "ffw") {
    const listing = load.data.queries.find(([key]) => key[0] === "shop-item");
    track(
      "ffw_shop_landing",
      { item_id: `${params.contractAddress ?? ""}-${params.itemId ?? ""}`.toLowerCase(), nid: url.searchParams.get("nid") ?? "", on_sale: !!listing?.[1] },
      { sid: session.sid, story: "notifications/first-wear"},
    );
  }
  return session.wrap({ ...load.data, sid: session.sid }, { status: load.status });
}
