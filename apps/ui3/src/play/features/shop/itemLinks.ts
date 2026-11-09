// Where a catalogue item leads: the URN a try-on puts on the avatar and its page on the web Shop; an
// outfit's page there; and a NAME's.

import type { CatalogItem } from '../../../shop/discovery/lib/catalogItem'
import { itemUrn } from '../../../shop/detail/lib/urn'
import { landNetwork } from '../../../shop/land/contracts'
import { CHAIN } from '../../../shop/model/chain'
import { nameTokenIdOf } from '../../../shop/names/names'
import { siteUrl } from '../../lib/baseDomain'
import { withUtm } from '../../lib/utm'

/** The item a try-on puts on: its own URN, else its collections-v2 URN where one exists (Polygon). */
export function tryOnUrn(item: CatalogItem): string | null {
  if (item.urn) return item.urn
  return item.chainId === CHAIN.POLYGON || item.chainId === CHAIN.AMOY ? itemUrn(item) : null
}

/** The item's page on the web Shop, where a resale or a MANA-only sale is bought; none without an item id. */
export function webShopUrl(item: CatalogItem): string | null {
  if (!item.itemId) return null
  return withUtm(siteUrl(`/shop/item/${encodeURIComponent(item.contractAddress.toLowerCase())}/${encodeURIComponent(item.itemId)}`))
}

/** An outfit's page on the web Shop, where it is bought when the Shop here can't buy it. */
export function webOutfitUrl(outfitId: string): string {
  return withUtm(siteUrl(`/shop/items/outfits/${encodeURIComponent(outfitId)}`))
}

/** The web Shop's NAMEs page on a typed NAME, where it is registered when the Shop here can't register it. */
export function webNameUrl(name: string): string {
  return withUtm(siteUrl(`/shop/names?name=${encodeURIComponent(name)}`))
}

/** A NAME's page on the web Shop (the Marketplace's), where its resale is bought. */
export function webNameTokenUrl(contract: string, tokenId: string): string {
  return withUtm(siteUrl(`/shop/token/${encodeURIComponent(contract.toLowerCase())}/${encodeURIComponent(tokenId)}`))
}

/** A registered NAME's bid page on the web Shop: an offer to its owner, whether or not they list it. */
export function webNameOfferUrl(name: string): string {
  return withUtm(siteUrl(`/shop/token/${landNetwork().nameRegistrar}/${nameTokenIdOf(name)}/bid`))
}
