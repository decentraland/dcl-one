// A scene's request to open the purchase confirmation for one item (the engine's item purchase
// request). The session answers on receipt, because the engine holds the scene's call, and every
// other scene's request, until it knows whether the confirmation can show here: `unavailable`
// where the host cannot buy, else `accepted` and then how the purchase ended. Every accepted
// request gets exactly one outcome, whatever the host's purchase does. A request naming no purchasable item fails where one could
// have been bought, so a scene learns the same from a bad urn on every host.
import type { ItemPurchaseItem, ItemPurchaseState } from '@explorer-core/engine/generated'
import type { ItemPurchaseRequestMessage } from '@explorer-core/engine/protocol'

/** How a purchase a scene asked for ended: what the scene is told. */
export type ItemPurchaseOutcome = Extract<ItemPurchaseState, 'purchased' | 'dismissed' | 'failed'>

/** Opens the host's purchase confirmation for `item` and resolves with how it ended. */
export type PurchaseSceneItem = (item: ItemPurchaseItem) => Promise<ItemPurchaseOutcome>

export function answerItemPurchase(
  request: ItemPurchaseRequestMessage,
  report: (id: number, state: ItemPurchaseState) => void,
  purchase: PurchaseSceneItem | undefined
): void {
  const { id, item } = request
  if (purchase == null) {
    report(id, 'unavailable')
    return
  }
  report(id, 'accepted')
  if (item == null) {
    report(id, 'failed')
    return
  }
  // inside the promise, so a host function that throws before returning one is a failure too
  new Promise<ItemPurchaseOutcome>((resolve) => resolve(purchase(item))).then(
    (outcome) => report(id, outcome),
    (e: unknown) => {
      console.warn('[item purchase] the purchase could not run', e)
      report(id, 'failed')
    }
  )
}
