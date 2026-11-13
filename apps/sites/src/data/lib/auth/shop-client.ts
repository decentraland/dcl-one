import { createShopClient, hostApiOptions, identitySigner, setShopClient } from "@ui/shop/api";
import type { AuthIdentity } from "@ui/data/auth/identity";

import { getIdentity } from "./session";

let installed = false;

/** The document's Shop client signs with sites' current identity, read per request (the top bar's credits too). */
export function installSitesShopClient(): void {
  if (installed || typeof window === "undefined") return;
  installed = true;
  setShopClient(createShopClient(hostApiOptions({ signer: () => identitySigner(getIdentity() as AuthIdentity | null) })));
}
