// The in-world Shop's purchases, through the web shell that holds the buyer's wallet (lib/shopProtocol):
// what the session can pay with, its Credits balance, and buying an item. The HUD only asks: the shell
// reads the terms, shows the purchase dialog over the app, signs and settles, and tells the HUD how it
// went, so the balance and the listings are read again once something is bought. Outside the shell (the
// UI-only mock, a credentialless embed) nothing can be bought here, and the page sends the buyer to the
// web Shop instead.

import { queryOptions, useQuery, useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useRef, useState } from 'react'
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
  type ShopOperation
} from '../../lib/shopProtocol'

/** Reading the balance signs a request in the shell; give it longer than an identity answer. */
const BALANCE_TIMEOUT_MS = 15_000

export const purchaseQueries = {
  capabilities: () =>
    queryOptions({
      queryKey: ['shop', 'purchase', 'capabilities'] as const,
      queryFn: () => shellRequest<ShopCapabilities>(SHOP_METHODS.capabilities),
      staleTime: 30_000,
      enabled: inShell
    }),
  balance: (buyer: string | null) =>
    queryOptions({
      queryKey: ['shop', 'credits', buyer] as const,
      queryFn: () => shellRequest<ShopBalance | null>(SHOP_METHODS.balance, undefined, BALANCE_TIMEOUT_MS),
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
  /** The last operation the shell reported, until another starts. */
  operation: ShopOperation | null
  /** Opens the shell's purchase dialog for `item`; `input` is how BUY was pressed. */
  buy(item: CatalogItem, creatorName?: string, input?: 'pointer' | 'keyboard'): void
}

/** Buying through the shell; `onBackpack` runs when the buyer leaves the success dialog for the Backpack. */
export function useShopPurchase(onBackpack: () => void): ShopPurchase {
  const queryClient = useQueryClient()
  const [operation, setOperation] = useState<ShopOperation | null>(null)
  const toBackpack = useRef(onBackpack)
  toBackpack.current = onBackpack
  const last = useRef<ShopOperation | null>(null)
  useEffect(() => {
    if (!inShell) return
    // A reloaded app picks up where the shell is: its newest operation (a purchase still settling
    // reports to it as it ends).
    shellRequest<ShopOperation[]>(SHOP_METHODS.status, {})
      .then((list) => {
        const newest = Array.isArray(list) ? list[0] : undefined
        if (newest && last.current == null) {
          last.current = newest
          setOperation(newest)
        }
      })
      .catch(() => {})
    return onShellMessage(SHOP_EVENT, (message) => {
      const event = parseOperationEvent(message)
      if (!event) return
      const prev = last.current
      last.current = event
      setOperation(event)
      // Bought: the balance and the listings' supply changed. Once, as the purchase succeeds.
      if (event.state === 'succeeded' && !(prev?.operation === event.operation && prev.state === 'succeeded')) {
        void queryClient.invalidateQueries({ queryKey: ['shop', 'credits'] })
        void queryClient.invalidateQueries({ queryKey: ['shop', 'catalogue'] })
        void queryClient.invalidateQueries({ queryKey: ['shop', 'home'] })
      }
      if (event.next === 'backpack' && prev?.next !== 'backpack') toBackpack.current()
    })
  }, [queryClient])
  const buy = useCallback((item: CatalogItem, creatorName?: string, input?: 'pointer' | 'keyboard') => {
    if (!inShell || item.itemId == null) return
    const request: ShopBuyRequest = {
      v: SHOP_PROTOCOL,
      item: { contract: item.contractAddress, itemId: String(item.itemId) },
      method: 'credits',
      ...(creatorName ? { creatorName } : {}),
      ...(input ? { input } : {})
    }
    shellRequest<ShopOperation>(SHOP_METHODS.buy, request)
      .then(setOperation)
      .catch((e: unknown) => console.warn('[shop] the purchase could not open', e))
  }, [])
  return { available: inShell, operation, buy }
}
