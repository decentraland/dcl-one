// from: shop@94584a5c:app/src/lib/jump.ts
// The in-world entry points offered after a purchase. The iOS app's web view cannot run the launcher
// page, so it gets the backpack deep link instead, opened on the first item that has a URN.

import { CHAIN } from "../../model/chain";

export function jumpUrl(chainId: number = CHAIN.POLYGON): string {
  return chainId === CHAIN.AMOY ? "https://decentraland.zone/jump" : "https://decentraland.org/jump";
}

export function backpackDeepLink(items: Array<{ urn?: string | null }>): string {
  const urn = items.find((i) => i.urn)?.urn;
  return `decentraland://open?iap_enabled=true${urn ? `&urn=${encodeURIComponent(urn)}` : ""}`;
}
