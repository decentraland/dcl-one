// The shell's record of in-world purchases, per buyer, in this browser's storage: each operation's state,
// a copy of its checkout progress, what its dialog shows, and its own cart and checkout (the web Shop's
// cart store, under the operation's own prefix, apart from the web Shop's cart). A purchase handed off to
// the wallet or the relay outlives the dialog, the app frame and the page: a reload restores it and
// watches it to its end (./purchase), and its state answers shop.status for as long as it is kept.
//
// Writes are observable: the web Shop's cart store swallows a failed write, so every write here goes
// through a wrapper that remembers which operation it failed for, and `saved` reads an operation back
// from storage. The purchase requires both before it lets anything reach the wallet or the relay.

import { CART_KEY_PREFIX, CHECKOUT_KEY_PREFIX, createCartStore, type CartStorage, type CartStore } from '../../../shop/cart/store'
import type { GroupProgress, GroupStatus } from '../../../shop/model/purchase'
import type { ShopOperationState } from '../../lib/shopProtocol'

/** What a purchase's dialog shows of its item. */
export interface PurchaseDisplay {
  name: string
  creator: string
  rarity: string
  thumbnail?: string
  category?: string
  credits: number
}

export interface OperationRecord {
  id: string
  /** Lowercase; null while signed out (never stored). */
  buyer: string | null
  item: { contract: string; itemId: string }
  state: ShopOperationState
  next?: 'backpack'
  txHash?: string | null
  /** The chain it settles on, kept before anything is reserved: a known hash is read there. */
  chainId?: number
  /** The checkout group's progress as the executor last wrote it, kept apart from the checkout itself. */
  progress?: GroupProgress
  display?: PurchaseDisplay
  createdAt: number
  updatedAt: number
}

const NS = 'dcl-inworld-shop'
const indexKey = (buyer: string): string => `${NS}:operations:v1:${buyer}`
const opPrefix = (id: string): string => `${NS}:operation:v1:${id}:`
const PROBE = `${NS}:probe`
/** Operations kept per buyer; the oldest settled ones go first. */
const KEEP = 20

// What storage hands back is checked whole (another version of this page, or anything on the origin,
// may have written it): a record that doesn't match is skipped, never half-trusted.
const STATES: readonly ShopOperationState[] = ['reviewing', 'processing', 'pending', 'succeeded', 'failed', 'cancelled']
const GROUP_STATUSES: readonly GroupStatus[] = ['idle', 'authorizing', 'submitted', 'pending', 'confirmed', 'failed', 'expired']
const ADDRESS = /^0x[0-9a-f]{40}$/
const HASH = /^0x[0-9a-fA-F]{64}$/
const UINT = /^(0|[1-9][0-9]{0,77})$/

const isObject = (v: unknown): v is Record<string, unknown> => v != null && typeof v === 'object' && !Array.isArray(v)
const isStrings = (v: unknown): v is string[] => Array.isArray(v) && v.every((s) => typeof s === 'string')
const isTime = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v) && v >= 0
const isChain = (v: unknown): v is number => typeof v === 'number' && Number.isSafeInteger(v) && v > 0
const isHash = (v: unknown): boolean => v === null || (typeof v === 'string' && HASH.test(v))

function isProgress(v: unknown): v is GroupProgress {
  if (!isObject(v) || !isObject(v.key)) return false
  const { key, failure } = v
  return (
    typeof v.keyId === 'string' &&
    (key.method === 'credits' || key.method === 'mana') &&
    isChain(key.chainId) &&
    typeof key.contract === 'string' &&
    isObject(key.call) &&
    typeof key.call.kind === 'string' &&
    isStrings(v.lineIds) &&
    GROUP_STATUSES.includes(v.status as GroupStatus) &&
    isStrings(v.creditIds) &&
    isHash(v.txHash) &&
    (failure === null || (isObject(failure) && typeof failure.kind === 'string' && typeof failure.message === 'string')) &&
    isTime(v.updatedAt) &&
    typeof v.attempts === 'number'
  )
}

function isDisplay(v: unknown): boolean {
  return (
    isObject(v) &&
    typeof v.name === 'string' &&
    typeof v.creator === 'string' &&
    typeof v.rarity === 'string' &&
    typeof v.credits === 'number' &&
    (v.thumbnail === undefined || typeof v.thumbnail === 'string') &&
    (v.category === undefined || typeof v.category === 'string')
  )
}

/** A stored operation of `buyer`'s, every field the shape this page writes. */
function isRecord(v: unknown, buyer: string): v is OperationRecord {
  if (!isObject(v) || !isObject(v.item)) return false
  const { item } = v
  return (
    typeof v.id === 'string' &&
    /^[0-9a-f-]{8,64}$/.test(v.id) &&
    v.buyer === buyer &&
    typeof item.contract === 'string' &&
    ADDRESS.test(item.contract) &&
    typeof item.itemId === 'string' &&
    UINT.test(item.itemId) &&
    STATES.includes(v.state as ShopOperationState) &&
    (v.next === undefined || v.next === 'backpack') &&
    (v.txHash === undefined || isHash(v.txHash)) &&
    (v.chainId === undefined || isChain(v.chainId)) &&
    (v.progress === undefined || isProgress(v.progress)) &&
    (v.display === undefined || isDisplay(v.display)) &&
    isTime(v.createdAt) &&
    isTime(v.updatedAt)
  )
}

export interface Ledger {
  load(buyer: string): OperationRecord[]
  /** Writes the record; false when it didn't stick. */
  save(record: OperationRecord): boolean
  /** The record as storage holds it now, read back. */
  saved(buyer: string, id: string): OperationRecord | null
  /** Whether this browser takes a write at all right now. */
  writable(): boolean
  /** Whether any write for this operation failed. */
  failed(id: string): boolean
  /** The operation's own cart and checkout, persisted with it. */
  storeFor(record: Pick<OperationRecord, 'id' | 'buyer'>): CartStore
}

export function createLedger(storage: () => Storage | null): Ledger {
  const failures = new Set<string>()
  const read = (buyer: string): OperationRecord[] => {
    try {
      const raw = storage()?.getItem(indexKey(buyer))
      const list = raw ? (JSON.parse(raw) as unknown) : []
      return Array.isArray(list) ? list.filter((r): r is OperationRecord => isRecord(r, buyer)) : []
    } catch {
      return []
    }
  }
  const forget = (s: Storage, record: OperationRecord): void => {
    const p = opPrefix(record.id)
    for (const key of [CART_KEY_PREFIX + record.buyer, CHECKOUT_KEY_PREFIX + record.buyer]) s.removeItem(p + key)
  }
  /** The operation's prefix over storage, remembering a write that throws (the cart store hides it). */
  const watched = (s: Storage, id: string): CartStorage => {
    const prefix = opPrefix(id)
    return {
      getItem: (key) => s.getItem(prefix + key),
      setItem: (key, value) => {
        try {
          s.setItem(prefix + key, value)
        } catch (e) {
          failures.add(id)
          throw e
        }
      },
      removeItem: (key) => {
        try {
          s.removeItem(prefix + key)
        } catch (e) {
          failures.add(id)
          throw e
        }
      }
    }
  }
  return {
    load: read,
    save(record) {
      if (!record.buyer) return false
      const s = storage()
      if (!s) {
        failures.add(record.id)
        return false
      }
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
        return true
      } catch (e) {
        failures.add(record.id)
        console.warn('[shell/shop] could not keep the purchase record', e)
        return false
      }
    },
    saved(buyer, id) {
      return read(buyer).find((r) => r.id === id) ?? null
    },
    writable() {
      const s = storage()
      if (!s) return false
      try {
        s.setItem(PROBE, '1')
        const ok = s.getItem(PROBE) === '1'
        s.removeItem(PROBE)
        return ok
      } catch {
        return false
      }
    },
    failed: (id) => failures.has(id),
    storeFor(record) {
      const s = storage()
      const store = createCartStore({
        storage: () => {
          if (s) return watched(s, record.id)
          failures.add(record.id)
          return null
        }
      })
      store.scopeTo(record.buyer)
      return store
    }
  }
}
