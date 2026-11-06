// The shell's record of in-world purchases, per buyer, in this browser's storage: each operation's state,
// and its own cart and checkout (the web Shop's cart store, under the operation's own prefix, apart from
// the web Shop's cart). A purchase handed off to the wallet or the relay outlives the dialog, the app
// frame and the page: a reload restores it and watches it to its end (./purchase), and its state answers
// shop.status for as long as it is kept.

import { CART_KEY_PREFIX, CHECKOUT_KEY_PREFIX, createCartStore, type CartStorage, type CartStore } from '../../../shop/cart/store'
import type { ShopOperationState } from '../../lib/shopProtocol'

export interface OperationRecord {
  id: string
  /** Lowercase; null while signed out (never stored). */
  buyer: string | null
  item: { contract: string; itemId: string }
  state: ShopOperationState
  next?: 'backpack'
  txHash?: string | null
  createdAt: number
  updatedAt: number
}

const NS = 'dcl-inworld-shop'
const indexKey = (buyer: string): string => `${NS}:operations:v1:${buyer}`
const opPrefix = (id: string): string => `${NS}:operation:v1:${id}:`
/** Operations kept per buyer; the oldest settled ones go first. */
const KEEP = 20

function prefixed(storage: Storage, prefix: string): CartStorage {
  return {
    getItem: (key) => storage.getItem(prefix + key),
    setItem: (key, value) => storage.setItem(prefix + key, value),
    removeItem: (key) => storage.removeItem(prefix + key)
  }
}

function isRecord(v: unknown): v is OperationRecord {
  const r = v as OperationRecord | null
  return !!r && typeof r.id === 'string' && typeof r.state === 'string' && !!r.item && typeof r.item.contract === 'string' && typeof r.item.itemId === 'string'
}

export interface Ledger {
  load(buyer: string): OperationRecord[]
  save(record: OperationRecord): void
  /** The operation's own cart and checkout, persisted with it. */
  storeFor(record: Pick<OperationRecord, 'id' | 'buyer'>): CartStore
}

export function createLedger(storage: () => Storage | null): Ledger {
  const read = (buyer: string): OperationRecord[] => {
    try {
      const raw = storage()?.getItem(indexKey(buyer))
      const list = raw ? (JSON.parse(raw) as unknown) : []
      return Array.isArray(list) ? list.filter(isRecord) : []
    } catch {
      return []
    }
  }
  const forget = (s: Storage, record: OperationRecord): void => {
    const p = opPrefix(record.id)
    for (const key of [CART_KEY_PREFIX + record.buyer, CHECKOUT_KEY_PREFIX + record.buyer]) s.removeItem(p + key)
  }
  return {
    load: read,
    save(record) {
      if (!record.buyer) return
      const s = storage()
      if (!s) return
      try {
        const list = read(record.buyer).filter((r) => r.id !== record.id)
        list.push({ ...record })
        // Settled operations beyond KEEP go, oldest first; an unsettled one is never dropped.
        const settled = list.filter((r) => r.state === 'succeeded' || r.state === 'failed' || r.state === 'cancelled').sort((a, b) => a.updatedAt - b.updatedAt)
        while (list.length > KEEP && settled.length > 0) {
          const old = settled.shift()!
          list.splice(list.indexOf(old), 1)
          forget(s, old)
        }
        s.setItem(indexKey(record.buyer), JSON.stringify(list))
      } catch (e) {
        console.warn('[shell/shop] could not keep the purchase record', e)
      }
    },
    storeFor(record) {
      const s = storage()
      const store = createCartStore({ storage: () => (s ? prefixed(s, opPrefix(record.id)) : null) })
      store.scopeTo(record.buyer)
      return store
    }
  }
}
