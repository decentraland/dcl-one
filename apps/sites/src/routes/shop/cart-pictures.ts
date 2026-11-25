import { cartPicturesScript } from "@ui/shop/cart/pictures";

import { headAccountScript } from "@/data/lib/auth/head-account";

/** The cart document's head script: the stored cart's first pictures start to load while the head parses. */
export function cartPicturesHeadScript(): string {
  return cartPicturesScript(headAccountScript());
}
