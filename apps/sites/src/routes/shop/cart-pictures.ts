import { cartPicturesScript } from "@ui/shop/cart/pictures";
import { CREATOR_HUB_HOST_KEY } from "@web-core/creator-hub-host";

import { SESSION_STORAGE_KEY } from "@/data/lib/auth/session";
import { SINGLE_SIGN_ON_PREFIX } from "@/data/lib/auth/single-sign-on";

/** The cart document's head script: the stored cart's first pictures start to load while the head parses. */
export const CART_PICTURES_SCRIPT = cartPicturesScript({ own: SESSION_STORAGE_KEY, shared: SINGLE_SIGN_ON_PREFIX, host: CREATOR_HUB_HOST_KEY });
