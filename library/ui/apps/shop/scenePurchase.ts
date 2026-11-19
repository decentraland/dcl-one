// A purchase a scene asked for (the engine's item purchase request, library/ui/apps/hud/App.tsx): one
// collection item, bought through the web shell as the in-world Shop's BUY is (./shopProtocol). The shell
// reads the terms and shows its purchase dialog; this only follows the operation to what the scene is told:
// purchased, dismissed or failed. A purchase handed off and still settling is `dismissed`: the scene has no
// later word, and `dismissed` claims nothing about whether the item was bought. A purchase the buyer
// starts again from the dialog (a changed price) is a new operation, and the scene's request ended with
// the first. When the shell answers with an earlier purchase of the item that is still settling, not one
// this request opened, the request is `dismissed` at once: following it would hold every scene's next
// request until that purchase settles, with its view already closed.

import { onShellMessage, shellRequest } from '@explorer-core/lib/shell'
import { parseOperationEvent, SHOP_EVENT, SHOP_METHODS, SHOP_PROTOCOL, type ShopBuyRequest, type ShopOperation, type ShopOperationState } from './shopProtocol'

export type SceneItemOutcome = 'purchased' | 'dismissed' | 'failed'

/** The shell loads its purchase code on the first shop request; give opening the dialog that long. */
export const BUY_TIMEOUT_MS = 30_000
/** The request itself is given longer, so a silent shell is told apart from one that refuses. */
const REQUEST_TIMEOUT_MS = BUY_TIMEOUT_MS + 5_000

/** What the scene is told as an operation reaches `state`; null while its dialog is still deciding. */
export function sceneOutcome(state: ShopOperationState): SceneItemOutcome | null {
  switch (state) {
    case 'succeeded':
      return 'purchased'
    case 'failed':
      return 'failed'
    case 'cancelled':
    case 'pending':
      return 'dismissed'
    default:
      return null
  }
}

/** How this reaches the shell; the tests' own in its place. */
export interface SceneShell {
  request: typeof shellRequest
  listen: typeof onShellMessage
}

/**
 * Opens the shell's purchase dialog for one unit of `item`, paid in Credits, and resolves with how it ended.
 * `failed` when the shell refused to open the dialog: it won't take the item, or another purchase is open.
 * `dismissed` when the shell did not answer in time: it may still open the dialog once it has loaded, and
 * `dismissed` claims nothing about a purchase made there.
 */
export function purchaseSceneItem(item: { contract: string; itemId: string }, shell: SceneShell = { request: shellRequest, listen: onShellMessage }): Promise<SceneItemOutcome> {
  return new Promise((resolve) => {
    // The operation's events can arrive before the answer that names it.
    let operation: string | null = null
    const early: ShopOperation[] = []
    const stop = shell.listen(SHOP_EVENT, (message) => {
      const op = parseOperationEvent(message)
      if (!op) return
      if (operation == null) early.push(op)
      else follow(op)
    })
    const end = (outcome: SceneItemOutcome): void => {
      clearTimeout(silent)
      stop()
      resolve(outcome)
    }
    const silent = setTimeout(() => end('dismissed'), BUY_TIMEOUT_MS)
    const follow = (op: { operation: string; state: ShopOperationState }): void => {
      if (op.operation !== operation) return
      const outcome = sceneOutcome(op.state)
      if (outcome) end(outcome)
    }
    const request: ShopBuyRequest = { v: SHOP_PROTOCOL, items: [{ contract: item.contract.toLowerCase(), itemId: item.itemId, quantity: 1 }], method: 'credits' }
    shell.request<ShopOperation>(SHOP_METHODS.buy, request, REQUEST_TIMEOUT_MS).then(
      (opened) => {
        const parsed = opened != null && typeof opened === 'object' ? parseOperationEvent({ ...opened, type: SHOP_EVENT }) : null
        if (!parsed) return end('failed')
        // A purchase this request opened starts in review; anything else is an earlier one of the item.
        if (parsed.state !== 'reviewing') return end(sceneOutcome(parsed.state) ?? 'dismissed')
        clearTimeout(silent)
        operation = parsed.operation
        for (const op of early) follow(op)
      },
      (e: unknown) => {
        console.warn('[shop] the purchase a scene asked for could not open', e)
        end('failed')
      }
    )
  })
}
