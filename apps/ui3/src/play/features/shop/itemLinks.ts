// Where a catalogue item leads: the URN a try-on puts on the avatar and its page on the web Shop.

import type { CatalogItem } from '../../../shop/discovery/lib/catalogItem'
import { itemUrn } from '../../../shop/detail/lib/urn'
import { CHAIN } from '../../../shop/model/chain'
import { siteUrl } from '../../lib/baseDomain'
import { withUtm } from '../../lib/utm'

/** The item a try-on puts on: its own URN, else its collections-v2 URN where one exists (Polygon). */
export function tryOnUrn(item: CatalogItem): string | null {
  if (item.urn) return item.urn
  return item.chainId === CHAIN.POLYGON || item.chainId === CHAIN.AMOY ? itemUrn(item) : null
}

/** The item's page on the web Shop, where a resale or a MANA-only sale is bought. */
export const webShopUrl = (item: CatalogItem): string => withUtm(siteUrl(`/shop/item/${item.contractAddress.toLowerCase()}/${item.itemId ?? ''}`))
