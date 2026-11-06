// The in-world Shop's purchases, through the web shell that holds the buyer's wallet (lib/shopProtocol):
// what the session can pay with, its Credits balance, and buying an item or the cart's items. The HUD
// only asks: the shell reads the terms, shows the purchase dialog over the app, signs and settles, and
// tells the HUD how it went, so the balance and the listings are read again once something is bought,
// and what the cart bought leaves it. Outside the shell (the UI-only mock, a credentialless embed)
// nothing can be bought here, and the page sends the buyer to the web Shop instead.

import { queryOptions, useQuery, useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useRef } from 'react'
import type { CatalogItem } from '../../../shop/discovery/lib/catalogItem'
import { inShell, onShellMessage, shellRequest } from '../../lib/shell'
import {
  parseOperationEvent,
  SHOP_EVENT,
  SHOP_METHODS,
  SHOP_PROTOCOL,
  type ShopBalance,
  type ShopBuyRequest,
  type ShopCapabilities,
  type ShopLine,
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

/** What the session can pay with in the Shop (undefined until the shell answers, or outside it). */
export function useShopCapabilities(): ShopCapabilities | undefined {
  return useQuery(purchaseQueries.capabilities()).data
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
}

/**
 * What a purchase from the cart bought leaves its buyer's cart (each operation is the part of the cart one
 * transaction bought), once per operation however often it is heard.
 */
function leaveCart(op: ShopOperation): void {
  if (op.state === 'succeeded' && op.cart && op.buyer) inWorldCart.applyPurchase(op.buyer, op.operation, op.items)
}

/** Buying through the shell; `onBackpack` runs when the buyer leaves the success dialog for the Backpack. */
export function useShopPurchase(onBackpack: () => void): ShopPurchase {
  const queryClient = useQueryClient()
  const toBackpack = useRef(onBackpack)
  toBackpack.current = onBackpack
  const last = useRef<ShopOperation | null>(null)
  useEffect(() => {
    if (!inShell) return
    // A reloaded app picks up where the shell is: its newest operation (a purchase still settling
    // reports to it as it ends), and what its cart's purchases bought while the app wasn't listening.
    shellRequest<ShopOperation[]>(SHOP_METHODS.status, {})
      .then((list) => {
        if (!Array.isArray(list)) return
        const known = list.filter((op) => op?.v === SHOP_PROTOCOL)
        if (known[0] && last.current == null) last.current = known[0]
        for (const op of known) {
          const parsed = parseOperationEvent({ ...op, type: SHOP_EVENT })
          if (parsed) leaveCart(parsed)
        }
      })
      .catch(() => {})
    return onShellMessage(SHOP_EVENT, (message) => {
      const event = parseOperationEvent(message)
      if (!event) return
      const prev = last.current
      last.current = event
      // Bought: the balance and the listings' supply changed. Once, as the purchase succeeds.
      if (event.state === 'succeeded' && !(prev?.operation === event.operation && prev.state === 'succeeded')) {
        void queryClient.invalidateQueries({ queryKey: ['shop', 'credits'] })
        void queryClient.invalidateQueries({ queryKey: ['shop', 'catalogue'] })
        void queryClient.invalidateQueries({ queryKey: ['shop', 'home'] })
      }
      leaveCart(event)
      if (event.next === 'backpack' && prev?.next !== 'backpack') toBackpack.current()
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
  return { available: inShell, buy, checkout }
}
