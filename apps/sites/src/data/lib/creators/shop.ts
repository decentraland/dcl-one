// Deep links into the Decentraland Shop, where the items of an approved collection are sold.
import { config } from '~/config'

/** The item's page in the Shop, keyed by collection contract and on-chain item id like the Shop's own route. */
export function shopItemUrl(contractAddress: string, itemId: string): string {
  return `${config.get('SHOP_URL')}/item/${encodeURIComponent(contractAddress)}/${encodeURIComponent(itemId)}`
}

export function shopCollectionUrl(contractAddress: string): string {
  return `${config.get('SHOP_URL')}/collection/${encodeURIComponent(contractAddress)}`
}

/** The creator's store in the Shop: every item they sell. */
export function shopCreatorUrl(address: string): string {
  return `${config.get('SHOP_URL')}/items/creator/${encodeURIComponent(address.toLowerCase())}`
}
