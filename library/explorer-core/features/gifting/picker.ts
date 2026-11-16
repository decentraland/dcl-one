// The gift picker's rules, as unity-explorer's gifting grid and inventory service apply them:
//  - a token sent as a gift is pending while the owned catalog still lists it with the `transferredAt` it had when
//    it was sent; it no longer counts towards the item's copies;
//  - an item with no copies left is not listed; an equipped one is listed, and can be picked only with a spare copy;
//  - the token a gift sends is the newest-transferred copy that is neither equipped nor pending.
// Only a Polygon collection item (collections-v2) can be sent, gaslessly; any other is refused with unity's message.

import type { GiftableItem, GiftableToken } from '../../engine/protocol'
import type { PendingGift } from '../../hostContract'

export const GIFT_PAGE_SIZE = 16
export const NOT_TRANSFERABLE = "Cannot send this item as a gift because it's not a transferable NFT."
export const DEFAULT_INFO = 'Gifting an item cannot be undone.'

export interface GiftRow {
  item: GiftableItem
  /** The copies left to gift: owned, less pending. */
  amount: number
  equipped: boolean
  /** False for an equipped item with no spare copy. */
  giftable: boolean
}

/** Whether a token urn is worn, by its full urn or as the item urn the look names. */
function worn(urn: string, equipped: ReadonlySet<string>): boolean {
  if (equipped.has(urn)) return true
  for (const e of equipped) if (e.startsWith(`${urn}:`)) return true
  return false
}

function isPending(token: GiftableToken, pending: readonly PendingGift[]): boolean {
  return pending.some((p) => p.urn === token.urn && token.transferredAt <= p.transferredAt)
}

/** The items the picker lists, in the order given. */
export function giftRows(items: readonly GiftableItem[], equippedUrns: readonly string[], pending: readonly PendingGift[]): GiftRow[] {
  const equipped = new Set(equippedUrns)
  const rows: GiftRow[] = []
  for (const item of items) {
    const pendingCount = item.tokens.filter((t) => isPending(t, pending)).length
    const amount = item.amount - pendingCount
    if (amount <= 0) continue
    const isEquipped = worn(item.urn, equipped) || item.tokens.some((t) => worn(t.urn, equipped))
    rows.push({ item, amount, equipped: isEquipped, giftable: !isEquipped || amount > 1 })
  }
  return rows
}

/** The token a gift of this item sends: the newest-transferred copy neither worn nor pending; null when none. */
export function bestToken(item: GiftableItem, equippedUrns: readonly string[], pending: readonly PendingGift[]): GiftableToken | null {
  const equipped = new Set(equippedUrns)
  let best: GiftableToken | null = null
  for (const t of item.tokens) {
    if (worn(t.urn, equipped) || isPending(t, pending)) continue
    if (best == null || t.transferredAt > best.transferredAt) best = t
  }
  return best
}

/** Whether a token urn names a Polygon collection item, the only kind a gift can send. */
export function transferable(urn: string): boolean {
  const parts = urn.split(':')
  return parts.length === 7 && parts[0] === 'urn' && parts[1] === 'decentraland' && (parts[2] === 'matic' || parts[2] === 'amoy') && parts[3] === 'collections-v2'
}

/** The footer's line: the warning, or what is about to be sent to whom. */
export function footerInfo(itemName: string | null, recipient: string): string {
  return itemName ? `You are about to send ${itemName} to ${recipient}` : DEFAULT_INFO
}
