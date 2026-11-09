// from: shop@94584a5c:app/src/lib/urn.ts
// Item URNs for the preview, and the Catalyst that holds a URN list's content. The v2 node serves
// its own content, so the peer is the node (the host's base) unless the host names one per network
// (the browser host reads `window.__SHOP_PEERS__ = { amoy: "https://peer.decentraland.zone" }`: api/host.ts).

import type { CatalogItem } from "../../discovery/lib/catalogItem";
import { CHAIN } from "../../model/chain";

export type UrnNetwork = "matic" | "amoy";

export function itemUrn(item: Pick<CatalogItem, "contractAddress" | "itemId" | "chainId">): `urn:decentraland:${string}` | null {
  if (!item.itemId) return null;
  const net = item.chainId === CHAIN.AMOY ? "amoy" : "matic";
  return `urn:decentraland:${net}:collections-v2:${item.contractAddress.toLowerCase()}:${item.itemId}`;
}

/** A collections-v2 item URN as the catalogue's `contract-itemId`, or null for anything else. */
export function itemIdFromUrn(urn: string): string | null {
  const match = urn.match(/collections-v2:(0x[0-9a-fA-F]{40}):(\d+)$/);
  return match ? `${match[1]!.toLowerCase()}-${match[2]}` : null;
}

/** The network a URN list belongs to: the first item URN decides; off-chain URNs never do. */
export function urnNetwork(urns: string[], fallback: UrnNetwork = "matic"): UrnNetwork {
  for (const urn of urns) {
    if (urn.includes(":amoy:")) return "amoy";
    if (urn.includes(":matic:")) return "matic";
  }
  return fallback;
}

/** The peers a host offers: one per network it names, else its base. */
export type ShopPeers = { byNetwork: Partial<Record<UrnNetwork, string>>; base: string };

export function peerUrlFor(urns: string[], peers: ShopPeers): string {
  return (peers.byNetwork[urnNetwork(urns)] ?? peers.base).replace(/\/$/, "");
}
