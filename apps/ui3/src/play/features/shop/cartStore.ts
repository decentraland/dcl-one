// The in-world Shop's cart (the design's Cart flow): what the buyer added with ADD TO CART and how many of
// each, in this browser. It is its own, apart from the web Shop's cart (another design, another store).
// Each line keeps what identifies the item and a snapshot to draw it with (its name, art and the card's
// price when it was added), never a price authority: CHECKOUT sends only the items and how many of each,
// and the shell reads the terms itself (shell/shop/purchase).
//
// Kept per account in localStorage (a signed-out cart under its own key, taken into the account's once
// one signs in). Storage that throws or is missing leaves the cart in memory for the page's life.
//
// What a purchase from the cart bought leaves the cart of the account that bought it (whichever is active
// now): as many of each item as it bought, once per operation, however often its success is heard (an
// event, the shell's status after a reload). The cart and the operations already taken out of it are one
// record per account, written whole in one go: a write that sticks keeps both, one that doesn't keeps
// neither, so a replay after a reload takes the purchase out exactly once either way. Within the page, the
// record a write couldn't keep is the one in memory.

import { useCallback, useEffect, useSyncExternalStore } from 'react'
import { cardPrice } from '../../../shop/discovery/cardPrice'
import type { CatalogItem } from '../../../shop/discovery/lib/catalogItem'
import { SIGNED_OUT } from '../../../shop/model/settlement'
import { MAX_PURCHASE_UNITS, type ShopLine } from '../../lib/shopProtocol'

export interface InWorldCartLine {
  /** Lowercase. */
  contract: string
  itemId: string
  quantity: number
  name: string
  thumbnail?: string
  rarity: string
  /** 'emote', or the wearable's category. */
  category?: string
  /** The creator's address. */
  creator: string
  /** The card's whole credits per unit when it was added: shown, never charged. */
  credits: number
  /** What was left of its supply then (null: unknown). */
  available: number | null
  addedAt: number
}

/** What ADD TO CART takes of an item. */
export type InWorldCartInput = Omit<InWorldCartLine, 'quantity' | 'addedAt'>

export type CartStorage = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>

const KEY_PREFIX = 'dcl-inworld-shop:cart:v2:'
const RECORD_VERSION = 2
/** Operations remembered as taken out of the cart (newest kept). */
const MAX_APPLIED = 100
const OPERATION_ID = /^[0-9a-f-]{8,64}$/
const GUEST = 'guest'
const ADDRESS = /^0x[0-9a-f]{40}$/
const UINT = /^(0|[1-9][0-9]{0,77})$/

const keyOf = (account: string | null): string => KEY_PREFIX + (account ?? GUEST)
const sameItem = (a: { contract: string; itemId: string }, b: { contract: string; itemId: string }): boolean => a.contract === b.contract && a.itemId === b.itemId
export const cartUnits = (lines: readonly { quantity: number }[]): number => lines.reduce((n, l) => n + l.quantity, 0)

function isLine(v: unknown): v is InWorldCartLine {
  if (v == null || typeof v !== 'object' || Array.isArray(v)) return false
  const l = v as Record<string, unknown>
  return (
    typeof l.contract === 'string' &&
    ADDRESS.test(l.contract) &&
    typeof l.itemId === 'string' &&
    UINT.test(l.itemId) &&
    typeof l.quantity === 'number' &&
    Number.isSafeInteger(l.quantity) &&
    l.quantity >= 1 &&
    l.quantity <= MAX_PURCHASE_UNITS &&
    typeof l.name === 'string' &&
    (l.thumbnail === undefined || typeof l.thumbnail === 'string') &&
    typeof l.rarity === 'string' &&
    (l.category === undefined || typeof l.category === 'string') &&
    typeof l.creator === 'string' &&
    typeof l.credits === 'number' &&
    Number.isFinite(l.credits) &&
    (l.available === null || (typeof l.available === 'number' && Number.isSafeInteger(l.available))) &&
    typeof l.addedAt === 'number'
  )
}

/** An account's cart: its lines, and the operations whose purchase already left it. */
interface CartRecord {
  lines: readonly InWorldCartLine[]
  applied: readonly string[]
}
const EMPTY: CartRecord = { lines: [], applied: [] }

/** A stored record in the shape this page writes, its lines each item once and within the cap; or null. */
function parseRecord(raw: string): CartRecord | null {
  const v = JSON.parse(raw) as unknown
  if (v == null || typeof v !== 'object' || Array.isArray(v)) return null
  const r = v as Record<string, unknown>
  if (r.v !== RECORD_VERSION || !Array.isArray(r.lines) || !Array.isArray(r.applied)) return null
  const lines: InWorldCartLine[] = []
  for (const line of r.lines.filter(isLine)) if (!lines.some((k) => sameItem(k, line)) && cartUnits(lines) + line.quantity <= MAX_PURCHASE_UNITS) lines.push(line)
  const applied = r.applied.filter((id): id is string => typeof id === 'string' && OPERATION_ID.test(id)).slice(-MAX_APPLIED)
  return { lines, applied }
}

/** The most of one line a cart may hold: what is left of it, within the purchase's cap. */
function capFor(line: Pick<InWorldCartLine, 'available'>, others: number): number {
  return Math.max(0, Math.min(line.available ?? MAX_PURCHASE_UNITS, MAX_PURCHASE_UNITS - others))
}

export interface InWorldCart {
  /** The active account's lines (lowercase account; null signed out). */
  getSnapshot(): readonly InWorldCartLine[]
  subscribe(listener: () => void): () => void
  scopeTo(account: string | null): void
  /** One more of the item (a new line, or one more of its line). False when the cart can't take it. */
  add(input: InWorldCartInput): boolean
  setQuantity(item: { contract: string; itemId: string }, quantity: number): void
  remove(item: { contract: string; itemId: string }): void
  /**
   * What `operation`, a purchase from the cart by `buyer`, bought leaves that account's cart: as many of
   * each item as it bought. Once per operation; a repeat changes nothing.
   */
  applyPurchase(buyer: string, operation: string, items: readonly ShopLine[]): void
  /** Whether one more unit of the item fits. */
  canAdd(item: Pick<InWorldCartLine, 'contract' | 'itemId' | 'available'>): boolean
}

export function createInWorldCart(opts: { storage?: () => CartStorage | null; now?: () => number } = {}): InWorldCart {
  const storage = opts.storage ?? defaultStorage
  const now = opts.now ?? Date.now
  const listeners = new Set<() => void>()
  let account: string | null = null
  /** The active account's record; its lines are the snapshot. */
  let current: CartRecord = EMPTY
  /** Every account's record this page read or wrote. */
  const memory = new Map<string, CartRecord>()
  /** Records whose last write didn't stick: memory, not storage, holds them now. */
  const unsaved = new Set<string>()

  const read = (who: string | null): CartRecord => {
    const key = keyOf(who)
    if (unsaved.has(key)) return memory.get(key) ?? EMPTY
    try {
      const raw = storage()?.getItem(key)
      if (raw != null) {
        const record = parseRecord(raw) ?? EMPTY
        memory.set(key, record)
        return record
      }
    } catch {
      // Unreadable: what this page knows of it.
    }
    return memory.get(key) ?? EMPTY
  }
  /** The whole record, in one write. */
  const write = (who: string | null, next: CartRecord): void => {
    const key = keyOf(who)
    memory.set(key, next)
    try {
      const s = storage()
      if (!s) throw new Error('no storage')
      if (next.lines.length === 0 && next.applied.length === 0) s.removeItem(key)
      else s.setItem(key, JSON.stringify({ v: RECORD_VERSION, lines: next.lines, applied: next.applied }))
      unsaved.delete(key)
    } catch {
      // Storage refused: the record stays in memory for the page's life.
      unsaved.add(key)
    }
  }
  const commit = (lines: readonly InWorldCartLine[]): void => {
    current = { lines, applied: current.applied }
    write(account, current)
    listeners.forEach((l) => l())
  }

  current = read(null)

  const cart: InWorldCart = {
    getSnapshot: () => current.lines,
    subscribe(listener) {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    scopeTo(next) {
      const who = next?.toLowerCase() ?? null
      if (who === account) return
      account = who
      let mine = read(who)
      // A signed-out cart is the account's once it signs in: its lines join, within the cap.
      if (who != null) {
        const guest = read(null).lines
        if (guest.length > 0) {
          const merged = [...mine.lines]
          for (const g of guest) {
            const at = merged.findIndex((l) => sameItem(l, g))
            const others = cartUnits(merged) - (at >= 0 ? merged[at].quantity : 0)
            const quantity = Math.min((at >= 0 ? merged[at].quantity : 0) + g.quantity, capFor(g, others))
            if (quantity <= 0) continue
            if (at >= 0) merged[at] = { ...merged[at], quantity }
            else merged.push({ ...g, quantity })
          }
          mine = { lines: merged, applied: mine.applied }
          write(who, mine)
          write(null, EMPTY)
        }
      }
      current = mine
      listeners.forEach((l) => l())
    },
    canAdd(item) {
      const { lines } = current
      const line = lines.find((l) => sameItem(l, item))
      const others = cartUnits(lines) - (line?.quantity ?? 0)
      return (line?.quantity ?? 0) + 1 <= capFor(line ?? item, others)
    },
    add(input) {
      const item = { ...input, contract: input.contract.toLowerCase() }
      if (!cart.canAdd(item)) return false
      const { lines } = current
      const at = lines.findIndex((l) => sameItem(l, item))
      if (at >= 0) {
        const next = [...lines]
        // The newer snapshot (its price and stock as the listing reads them now), one more of it.
        next[at] = { ...lines[at], ...item, quantity: lines[at].quantity + 1 }
        commit(next)
      } else commit([...lines, { ...item, quantity: 1, addedAt: now() }])
      return true
    },
    setQuantity(item, quantity) {
      const { lines } = current
      const at = lines.findIndex((l) => sameItem(l, item))
      if (at < 0) return
      const line = lines[at]
      const q = Math.min(Math.max(1, Math.floor(quantity)), capFor(line, cartUnits(lines) - line.quantity))
      if (q < 1 || q === line.quantity) return
      const next = [...lines]
      next[at] = { ...line, quantity: q }
      commit(next)
    },
    remove(item) {
      const { lines } = current
      if (lines.some((l) => sameItem(l, item))) commit(lines.filter((l) => !sameItem(l, item)))
    },
    applyPurchase(buyer, operation, items) {
      const who = buyer.toLowerCase()
      const mine = who === account ? current : read(who)
      if (mine.applied.includes(operation)) return
      // What it bought and that it was taken out, together: one record, one write.
      const next: CartRecord = {
        lines: mine.lines.flatMap((l) => {
          const bought = items.reduce((n, i) => (sameItem(l, i) ? n + i.quantity : n), 0)
          if (bought === 0) return [l]
          return l.quantity > bought ? [{ ...l, quantity: l.quantity - bought }] : []
        }),
        applied: [...mine.applied, operation].slice(-MAX_APPLIED)
      }
      write(who, next)
      if (who !== account) return
      current = next
      listeners.forEach((l) => l())
    }
  }
  return cart
}

function defaultStorage(): CartStorage | null {
  try {
    return typeof localStorage !== 'undefined' ? localStorage : null
  } catch {
    return null
  }
}

/** The page's cart. */
export const inWorldCart = createInWorldCart()

/** The cart's lines for `account`, kept current. */
export function useInWorldCart(account: string | null, cart: InWorldCart = inWorldCart): readonly InWorldCartLine[] {
  const subscribe = useCallback((listener: () => void) => cart.subscribe(listener), [cart])
  useEffect(() => cart.scopeTo(account), [cart, account])
  return useSyncExternalStore(subscribe, cart.getSnapshot)
}

/** What ADD TO CART keeps of a listing: a primary sale in Credits only (anything else is bought on the web). */
export function cartInputFor(item: CatalogItem): InWorldCartInput | null {
  const price = cardPrice(item, SIGNED_OUT)
  if (price.kind !== 'credits' || item.itemId == null || !ADDRESS.test(item.contractAddress.toLowerCase())) return null
  return {
    contract: item.contractAddress.toLowerCase(),
    itemId: String(item.itemId),
    name: item.name,
    thumbnail: item.thumbnail || undefined,
    rarity: item.rarity || 'base',
    category: item.category === 'emote' ? 'emote' : (item.wearableCategory ?? item.category),
    creator: item.creator,
    credits: price.credits,
    available: item.available ?? null
  }
}

/** The lines as the purchase request names them. */
export function cartRequestLines(lines: readonly InWorldCartLine[]): ShopLine[] {
  return lines.map((l) => ({ contract: l.contract, itemId: l.itemId, quantity: l.quantity }))
}
