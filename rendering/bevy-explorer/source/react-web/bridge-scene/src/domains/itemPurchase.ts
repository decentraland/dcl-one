// Item purchase: relays a scene's request to open the purchase confirmation for one item
// (`openExplorerUi` with `EU_ITEM_PURCHASE`) to the React HUD, and the HUD's reports back: whether
// it can show the confirmation, then how it ended. The engine has applied its gates and parsed the
// item from the scene's urn; `item` is null when the urn names no purchasable collection item.
//
// The engine holds the request, and every other scene's, until the page reports, and it sees this
// scene's stream, not the page. So the request in flight is kept here and settled when a page says
// `hello` (it started late or reloaded): one the page never answered is sent again, and one a
// previous page accepted is reported `dismissed`, since the page that followed its purchase is gone
// and `dismissed` claims nothing about it. A request the engine has ended is forgotten when the
// stream is opened again.
//   from: SystemApi.getItemPurchaseRequestStream / setItemPurchaseState.
import { BevyApi } from '../bevy-api'
import { relay } from '../system-helpers'
import type { Ctx } from '../bridge'
import type { ItemPurchaseRequest } from '@explorer-core/engine/generated'

export function registerItemPurchase(ctx: Ctx): void {
  const open = BevyApi.getItemPurchaseRequestStream
  if (open == null) return
  let inFlight: { request: ItemPurchaseRequest; accepted: boolean } | null = null
  const send = (request: ItemPurchaseRequest): void => ctx.send({ kind: 'itemPurchaseRequest', id: request.id, item: request.item })

  relay(
    'itemPurchase',
    async () => {
      // A stream opened again is a new subscription: the engine ended whatever the last one left
      // in flight, so nothing of it is sent again.
      inFlight = null
      return await open()
    },
    (request) => {
      inFlight = { request, accepted: false }
      send(request)
    }
  )

  ctx.on('hello', () => {
    if (inFlight == null) return
    if (!inFlight.accepted) return send(inFlight.request)
    BevyApi.setItemPurchaseState?.({ id: inFlight.request.id, state: 'dismissed' })
    inFlight = null
  })

  ctx.on('itemPurchaseState', (msg) => {
    if (inFlight?.request.id === msg.id) {
      if (msg.state === 'accepted') inFlight.accepted = true
      else inFlight = null
    }
    BevyApi.setItemPurchaseState?.({ id: msg.id, state: msg.state })
  })
}
