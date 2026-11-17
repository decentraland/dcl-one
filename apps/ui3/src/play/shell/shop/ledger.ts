// The shell's record of in-world purchases, per buyer, in this browser's storage: each operation's state,
// a copy of its checkout progress, what its dialog shows, and its own cart and checkout (the web Shop's
// cart store, under the operation's own prefix, apart from the web Shop's cart). A purchase handed off to
// the wallet or the relay outlives the dialog, the app frame and the page: a reload restores it and
// watches it to its end (./purchase), and its state answers shop.status for as long as it is kept.
//
// Writes are observable: the web Shop's cart store swallows a failed write, so every write here goes
// through a wrapper that remembers which operation it failed for, and `saved` reads an operation back
// from storage. The purchase requires both before it lets anything reach the wallet or the relay.
//
// Each operation's record is a key of its own, written only by the tab that holds the operation's lock
// (./purchase): tabs running different purchases never write the same key, so neither can undo the
// other's checkpoint. A buyer's operations are found by their keys' prefix.

import { CART_KEY_PREFIX, CHECKOUT_KEY_PREFIX, createCartStore, type CartStorage, type CartStore } from '../../../shop/cart/store'
import type { GroupProgress, GroupStatus } from '@shop/model/purchase'
import { MAX_PURCHASE_UNITS, OPERATION_STATES, parseLines, TERMINAL_STATES, type ShopLine, type ShopOperationState } from '../../lib/shopProtocol'

/** What a purchase's dialog shows of one item it buys (one per unit: two of an item are two). */
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
  /** What it buys: the items of one checkout group (one transaction, one credit). */
  items: ShopLine[]
  /** Bought from the app's cart. */
  cart?: true
  state: ShopOperationState
  next?: 'backpack'
  txHash?: string | null
  /** The chain it settles on, kept before anything is reserved: a known hash is read there. */
  chainId?: number
  /** The checkout group's progress as the executor last wrote it, kept apart from the checkout itself. */
  progress?: GroupProgress
  /** One per unit it buys, in order. */
  displays?: PurchaseDisplay[]
  createdAt: number
  updatedAt: number
}

const NS = 'dcl-inworld-shop'
const recordPrefix = (buyer: string): string => `${NS}:record:v1:${buyer}:`
const recordKey = (buyer: string, id: string): string => recordPrefix(buyer) + id
const opPrefix = (id: string): string => `${NS}:operation:v1:${id}:`
/** Each ledger probes its own key, so two tabs probing at once don't answer for each other. */
const probeKey = (): string => `${NS}:probe:${Math.random().toString(36).slice(2)}`
/** Operations kept per buyer; the oldest settled ones go first. */
const KEEP = 20

// What storage hands back is checked whole (another version of this page, or anything on the origin,
// may have written it): a record that doesn't match is skipped, never half-trusted.
const GROUP_STATUSES: readonly GroupStatus[] = ['idle', 'authorizing', 'submitted', 'pending', 'confirmed', 'failed', 'expired']
const HASH = /^0x[0-9a-fA-F]{64}$/
const MAX_DISPLAYS = MAX_PURCHASE_UNITS

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

/**
 * A record written in the one-item shape (an `item` and its `display`, before purchases bought several),
 * in today's: one line of one unit, and one display. Anything else is left as it is, to be checked whole.
 */
function upgrade(v: unknown): unknown {
  if (!isObject(v) || !('item' in v) || 'items' in v) return v
  const { item, display, ...rest } = v
  return { ...rest, items: isObject(item) ? [{ ...item, quantity: 1 }] : item, ...(display !== undefined ? { displays: [display] } : {}) }
}

/** A stored operation of `buyer`'s, every field the shape this page writes. */
function isRecord(v: unknown, buyer: string): v is OperationRecord {
  if (!isObject(v)) return false
  // The lines as this page writes them: lowercase contracts, as parseLines gives them back.
  const lines = parseLines(v.items)
  return (
    typeof v.id === 'string' &&
    /^[0-9a-f-]{8,64}$/.test(v.id) &&
    v.buyer === buyer &&
    lines !== null &&
    lines.every((l, i) => (v.items as ShopLine[])[i].contract === l.contract) &&
    (v.cart === undefined || v.cart === true) &&
    OPERATION_STATES.includes(v.state as ShopOperationState) &&
    (v.next === undefined || v.next === 'backpack') &&
    (v.txHash === undefined || isHash(v.txHash)) &&
    (v.chainId === undefined || isChain(v.chainId)) &&
    (v.progress === undefined || isProgress(v.progress)) &&
    (v.displays === undefined || (Array.isArray(v.displays) && v.displays.length <= MAX_DISPLAYS && v.displays.every(isDisplay))) &&
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
  const PROBE = probeKey()
  /** The record under `key`, if it is `buyer`'s operation `id` in the shape this page writes. */
  const parse = (s: Storage, key: string, buyer: string, id: string): OperationRecord | null => {
    try {
      const raw = s.getItem(key)
      const v = raw ? upgrade(JSON.parse(raw) as unknown) : null
      return isRecord(v, buyer) && v.id === id ? v : null
    } catch {
      return null
    }
  }
  /** `buyer`'s operations, oldest first. */
  const read = (buyer: string): OperationRecord[] => {
    try {
      const s = storage()
      if (!s) return []
      const prefix = recordPrefix(buyer)
      const keys: string[] = []
      for (let i = 0; i < s.length; i++) {
        const key = s.key(i)
        if (key?.startsWith(prefix)) keys.push(key)
      }
      return keys
        .map((key) => parse(s, key, buyer, key.slice(prefix.length)))
        .filter((r): r is OperationRecord => r !== null)
        .sort((a, b) => a.createdAt - b.createdAt)
    } catch {
      return []
    }
  }
  const forget = (s: Storage, record: OperationRecord): void => {
    const p = opPrefix(record.id)
    for (const key of [CART_KEY_PREFIX + record.buyer, CHECKOUT_KEY_PREFIX + record.buyer]) s.removeItem(p + key)
    if (record.buyer) s.removeItem(recordKey(record.buyer, record.id))
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
        s.setItem(recordKey(record.buyer, record.id), JSON.stringify(record))
      } catch (e) {
        failures.add(record.id)
        console.warn('[shell/shop] could not keep the purchase record', e)
        return false
      }
      // Settled operations beyond KEEP go, oldest first; an unsettled one, or this one, is never dropped.
      try {
        const list = read(record.buyer)
        const settled = list.filter((r) => r.id !== record.id && TERMINAL_STATES.includes(r.state)).sort((a, b) => a.updatedAt - b.updatedAt)
        for (let count = list.length; count > KEEP && settled.length > 0; count--) forget(s, settled.shift()!)
      } catch {
        // A record left over is no harm.
      }
      return true
    },
    saved(buyer, id) {
      const s = storage()
      return s ? parse(s, recordKey(buyer, id), buyer, id) : null
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
