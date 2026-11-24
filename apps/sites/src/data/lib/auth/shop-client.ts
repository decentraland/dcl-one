import { getDocumentShopClient, setShopClient } from "@shop/api/document-client";
import { createShopClient, type ShopClient } from "@shop/api/shop";
import { fetchTransport, identitySigner, shopApiMode } from "@web-core/auth/shopRequest";

import { catalystUrl } from "../catalyst/bases";
import { getIdentity } from "./session";

let installed = false;

function createSitesShopClient() {
  return createShopClient({
    base: catalystUrl(import.meta.env.VITE_CATALYST_URL || undefined),
    mode: shopApiMode(),
    signer: () => identitySigner(getIdentity()),
    transport: fetchTransport(),
  });
}

/** The document and the Shop pages share one client signed with the current Sites identity. */
export function installSitesShopClient(): void {
  if (installed || typeof window === "undefined") return;
  installed = true;
  setShopClient(createSitesShopClient());
}

export function sitesShopClient(): ShopClient {
  if (typeof window === "undefined") return createSitesShopClient();
  installSitesShopClient();
  return getDocumentShopClient(createSitesShopClient);
}
