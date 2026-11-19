// The in-world Shop's cart in this page: the model (@shop/cart/inWorld) on the browser's localStorage, its
// storage events and Web Locks, and the React hook that reads it.

import { useCallback, useEffect, useSyncExternalStore } from 'react'
import {
  CART_LOCK,
  createInWorldCart as createCart,
  localCartLock,
  type CartLock,
  type CartStorage,
  type InWorldCart,
  type InWorldCartLine,
  type StorageWatch
} from '@shop/cart/inWorld'

export { cartRequestLines, cartUnits, inWorldCartInputFor } from '@shop/cart/inWorld'
export type { CartLock, CartStorage, InWorldCart, InWorldCartInput, InWorldCartLine, StorageWatch } from '@shop/cart/inWorld'

const windowStorageEvents: StorageWatch = (onChange) => {
  if (typeof window === 'undefined') return () => {}
  const listener = (e: StorageEvent): void => onChange(e.key)
  window.addEventListener('storage', listener)
  return () => window.removeEventListener('storage', listener)
}

/** Web Locks across the origin's tabs; without them, this page's own writes still go one at a time. */
function cartLock(): CartLock {
  const local = localCartLock()
  return <T,>(fn: () => T): Promise<T> => {
    const locks = typeof navigator !== 'undefined' ? navigator.locks : undefined
    // `fn` is synchronous: the lock is held from the read to the write, and goes as it returns.
    return locks ? (locks.request(CART_LOCK, () => fn()) as Promise<T>) : local(fn)
  }
}

function defaultStorage(): CartStorage | null {
  try {
    return typeof localStorage !== 'undefined' ? localStorage : null
  } catch {
    return null
  }
}

/** A cart on this page's storage, events and locks; a test passes its own. */
export function createInWorldCart(
  opts: { storage?: () => CartStorage | null; now?: () => number; watch?: StorageWatch; lock?: CartLock; newId?: () => string } = {}
): InWorldCart {
  return createCart({
    storage: opts.storage ?? defaultStorage,
    watch: opts.watch ?? windowStorageEvents,
    lock: opts.lock ?? cartLock(),
    newId: opts.newId ?? (() => crypto.randomUUID()),
    now: opts.now
  })
}

/** The page's cart. */
export const inWorldCart = createInWorldCart()

/** The cart's lines for `account`, kept current. */
export function useInWorldCart(account: string | null, cart: InWorldCart = inWorldCart): readonly InWorldCartLine[] {
  const subscribe = useCallback((listener: () => void) => cart.subscribe(listener), [cart])
  useEffect(() => cart.scopeTo(account), [cart, account])
  return useSyncExternalStore(subscribe, cart.getSnapshot)
}
