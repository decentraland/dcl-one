import Page from "@ui/shop/pages/Cart";

import type { HeadScriptHandle } from "../root";
import { cartPicturesHeadScript } from "../shop/cart-pictures";
import { shopLoader } from "../shop/loader.server";
import { ShopPage, shopClientLoader, shopHeaders, shopMeta } from "../shop/page";
import { sharedCartMiddleware } from "../shop/shared-cart";

import type { Route } from "./+types/shop.cart";

export const middleware: Route.MiddlewareFunction[] = [sharedCartMiddleware];
export const loader = ({ request, params, context }: Route.LoaderArgs) => shopLoader("cart", request, params, context);
export const clientLoader = shopClientLoader;
export const meta = shopMeta("cart");
export const headers = shopHeaders;
// Only the document that the server sends runs a head script (RouteHeadScripts), so the browser bundle carries an empty one.
export const handle: HeadScriptHandle = { headScript: import.meta.env.SSR ? cartPicturesHeadScript() : "" };

export default function ShopCartRoute({ loaderData }: Route.ComponentProps) {
  return (
    <ShopPage data={loaderData}>
      <Page />
    </ShopPage>
  );
}
