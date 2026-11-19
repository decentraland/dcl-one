import type { CatalogItem } from "../../discovery/lib/catalogItem";
import { CHAIN } from "../../model/chain";
import { itemUrn } from "./urn";

/** The item a try-on puts on: its own URN, else its collections-v2 URN where one exists (Polygon). */
export function tryOnUrn(item: CatalogItem): string | null {
  if (item.urn) return item.urn;
  return item.chainId === CHAIN.POLYGON || item.chainId === CHAIN.AMOY ? itemUrn(item) : null;
}
