// The in-world Shop's purchases, through the web shell that holds the buyer's wallet (lib/shopProtocol):
// what the session can pay with, its Credits balance, and buying an item, the cart's items or a NAME. The
// HUD only asks: the shell reads the terms, shows the purchase dialog over the app, signs and settles, and
// tells the HUD how it went, so the balance and the listings are read again once something is bought,
// and what the cart bought leaves it. Each operation's outcome and the buyer's choice from its success
// dialog are acted on once per operation, however often its event or shop.status repeats them, and the
// items' handling never acts on a NAME's. Outside the shell (the UI-only mock, a credentialless embed)
// nothing can be bought here, and the page sends the buyer to the web Shop instead.

import { queryOptions, useQuery, useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useRef } from 'react'
import type { CatalogItem } from '../../../shop/discovery/lib/catalogItem'
import { inShell, onShellMessage, shellRequest } from '../../lib/shell'
import {
  parseOperationEvent,
  SHOP_BUSY,
  SHOP_EVENT,
  SHOP_METHODS,
  SHOP_PROTOCOL,
  type ShopBalance,
  type ShopBuyNameRequest,
  type ShopBuyRequest,
  type ShopCapabilities,
  type ShopItemOperation,
  type ShopLine,
  type ShopNamesMode,
  type ShopOperation
} from '../../lib/shopProtocol'
import { inWorldCart } from './cartStore'
import type { PressedWith } from './ui/pressedWith'

/** Reading the balance signs a request in the shell; give it longer than an identity answer. */
const BALANCE_TIMEOUT_MS = 15_000
/** The shell loads its purchase code on the first shop request; give BUY that long too. */
const BUY_TIMEOUT_MS = 30_000

/** A shell answer in this protocol's version; another version's is no answer this HUD can read. */
function versioned<T extends { v: number } | null>(answer: T): T {
  if (answer != null && answer.v !== SHOP_PROTOCOL) throw new Error(`the shell answered shop protocol ${String(answer.v)}`)
  return answer
}

export const purchaseQueries = {
  capabilities: () =>
    queryOptions({
      queryKey: ['shop', 'purchase', 'capabilities'] as const,
      queryFn: () => shellRequest<ShopCapabilities>(SHOP_METHODS.capabilities).then(versioned),
      staleTime: 30_000,
      enabled: inShell
    }),
  balance: (buyer: string | null) =>
    queryOptions({
      queryKey: ['shop', 'credits', buyer] as const,
      queryFn: () => shellRequest<ShopBalance | null>(SHOP_METHODS.balance, undefined, BALANCE_TIMEOUT_MS).then(versioned),
      staleTime: 30_000,
      enabled: inShell && buyer != null
    })
}

/** The buyer's whole Credits, or undefined until known (signed out: never). */
export function useCreditsBalance(buyer: string | null): number | undefined {
  return useQuery(purchaseQueries.balance(buyer)).data?.credits
}

export interface ShopPurchase {
  /** Whether purchases run here (in the web shell). */
  available: boolean
  /**
   * Opens the shell's purchase dialog for `item`; `input` is how BUY was pressed. False when it couldn't
   * (the shell refused or didn't answer), for the page to offer the web Shop instead.
   */
  buy(item: CatalogItem, input?: PressedWith): Promise<boolean>
  /** Opens the shell's purchase dialog for the cart's lines (CHECKOUT); false when it couldn't. */
  checkout(lines: ShopLine[], input?: PressedWith): Promise<boolean>
  /** Opens it for several items bought together, not from the cart (an outfit's BUY); false when it couldn't. */
  buyAll(lines: ShopLine[], input?: PressedWith): Promise<boolean>
  /**
   * Opens the shell's BUY NAME dialog for `name`, in the mode the shell's capabilities said. `busy` when
   * another purchase's dialog is open (that dialog is what the buyer sees), `unavailable` when the shell
   * refused otherwise, didn't answer, or predates NAMEs, for the page to offer the web Shop instead.
   */
  buyName(name: string, mode: ShopNamesMode, input?: PressedWith): Promise<ShopNameOpening>
}

/** How asking the shell for BUY NAME went. */
export type ShopNameOpening = 'opened' | 'busy' | 'unavailable'

/** What the page does as a NAME's purchase ends. */
export interface ShopNameActions {
  /** A simulated NAME's PREVIEW ON AVATAR, by `buyer`: show it over the avatar here, saving nothing. */
  onPreviewName?(name: string, buyer: string | null): void
}

/**
 * What a purchase from the cart bought leaves its buyer's cart (each operation is the part of the cart one
 * transaction bought), once per operation however often it is heard.
 */
function leaveCart(op: ShopItemOperation): void {
  if (op.state === 'succeeded' && op.cart && op.buyer) void inWorldCart.applyPurchase(op.buyer, op.operation, op.items)
}

/** What an operation has come to that the page acts on, each once per operation: it succeeded, and the choice from its success dialog. */
function outcomes(op: ShopOperation): string[] {
  return [...(op.state === 'succeeded' ? [`${op.operation}:succeeded`] : []), ...(op.next ? [`${op.operation}:${op.next}`] : [])]
}

/**
 * Buying through the shell; `onBackpack` runs when the buyer leaves an item's success dialog for the
 * Backpack, and `names` says what to do as a NAME's purchase ends.
 */
export function useShopPurchase(onBackpack: () => void, names: ShopNameActions = {}): ShopPurchase {
  const queryClient = useQueryClient()
  const toBackpack = useRef(onBackpack)
  toBackpack.current = onBackpack
  const nameActions = useRef(names)
  nameActions.current = names
  useEffect(() => {
    if (!inShell) return
    /** Each operation's outcomes already acted on (or already past when the app came up). */
    const done = new Set<string>()
    const once = (key: string): boolean => {
      if (done.has(key)) return false
      done.add(key)
      return true
    }
    const act = (op: ShopOperation): void => {
      if (op.name !== undefined) {
        // A NAME's: no items left the cart or the listings, and a simulation spent nothing.
        if (op.mode === 'simulation' && op.next === 'previewName' && once(`${op.operation}:previewName`)) nameActions.current.onPreviewName?.(op.name, op.buyer)
        return
      }
      // Bought: the balance and the listings' supply changed. Once, as the purchase succeeds.
      if (op.state === 'succeeded' && once(`${op.operation}:succeeded`)) {
        void queryClient.invalidateQueries({ queryKey: ['shop', 'credits'] })
        void queryClient.invalidateQueries({ queryKey: ['shop', 'catalogue'] })
        void queryClient.invalidateQueries({ queryKey: ['shop', 'home'] })
      }
      leaveCart(op)
      if (op.next === 'backpack' && once(`${op.operation}:backpack`)) toBackpack.current()
    }
    // A reloaded app picks up where the shell is: what its operations came to before is past (a purchase
    // still settling reports to it as it ends), and what its cart's purchases bought while the app wasn't
    // listening leaves the cart.
    shellRequest<ShopOperation[]>(SHOP_METHODS.status, {})
      .then((list) => {
        if (!Array.isArray(list)) return
        for (const op of list) {
          const parsed = op != null && typeof op === 'object' ? parseOperationEvent({ ...op, type: SHOP_EVENT }) : null
          if (!parsed) continue
          for (const key of outcomes(parsed)) done.add(key)
          if (parsed.name === undefined) leaveCart(parsed)
        }
      })
      .catch(() => {})
    return onShellMessage(SHOP_EVENT, (message) => {
      const event = parseOperationEvent(message)
      if (event) act(event)
    })
  }, [queryClient])
  const request = useCallback(async (items: ShopLine[], cart: boolean, input?: PressedWith): Promise<boolean> => {
    if (!inShell || items.length === 0) return false
    const request: ShopBuyRequest = { v: SHOP_PROTOCOL, items, method: 'credits', ...(cart ? { cart: true as const } : {}), ...(input ? { input } : {}) }
    try {
      versioned(await shellRequest<ShopOperation>(SHOP_METHODS.buy, request, BUY_TIMEOUT_MS))
      return true
    } catch (e) {
      console.warn('[shop] the purchase could not open', e)
      return false
    }
  }, [])
  const buy = useCallback(
    (item: CatalogItem, input?: PressedWith): Promise<boolean> =>
      item.itemId == null ? Promise.resolve(false) : request([{ contract: item.contractAddress.toLowerCase(), itemId: String(item.itemId), quantity: 1 }], false, input),
    [request]
  )
  const checkout = useCallback((lines: ShopLine[], input?: PressedWith): Promise<boolean> => request(lines, true, input), [request])
  const buyAll = useCallback((lines: ShopLine[], input?: PressedWith): Promise<boolean> => request(lines, false, input), [request])
  const buyName = useCallback(async (name: string, mode: ShopNamesMode, input?: PressedWith): Promise<ShopNameOpening> => {
    if (!inShell) return 'unavailable'
    const request: ShopBuyNameRequest = { v: SHOP_PROTOCOL, name, method: 'credits', mode, ...(input ? { input } : {}) }
    try {
      versioned(await shellRequest<ShopOperation>(SHOP_METHODS.buyName, request, BUY_TIMEOUT_MS))
      return 'opened'
    } catch (e) {
      if (e instanceof Error && e.message === SHOP_BUSY) return 'busy'
      console.warn('[shop] the NAME purchase could not open', e)
      return 'unavailable'
    }
  }, [])
  return { available: inShell, buy, checkout, buyAll, buyName }
}
