// Item purchase: relays a scene's request to open the purchase confirmation for one item
// (`openExplorerUi` with `EU_ITEM_PURCHASE`) to the React HUD, and the HUD's reports back: whether
// it can show the confirmation, then how it ended. The engine has applied its gates and parsed the
// item from the scene's urn; `item` is null when the urn names no purchasable collection item.
//   from: SystemApi.getItemPurchaseRequestStream / setItemPurchaseState.
import { BevyApi } from '../bevy-api'
import { relay } from '../system-helpers'
import type { Ctx } from '../bridge'

export function registerItemPurchase(ctx: Ctx): void {
  const open = BevyApi.getItemPurchaseRequestStream
  if (open == null) return
  relay('itemPurchase', async () => await open(), (req) => {
    ctx.send({ kind: 'itemPurchaseRequest', id: req.id, item: req.item })
  })

  ctx.on('itemPurchaseState', (msg) => {
    BevyApi.setItemPurchaseState?.({ id: msg.id, state: msg.state })
  })
}
