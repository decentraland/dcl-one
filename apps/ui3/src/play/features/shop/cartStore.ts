// The in-world Shop's cart (the design's Cart flow): what the buyer added with ADD TO CART and how many of
// each, in this browser. It is its own, apart from the web Shop's cart (another design, another store).
// Each line keeps what identifies the item and a snapshot to draw it with (its name, art and the card's
// price when it was added), never a price authority: CHECKOUT sends only the items and how many of each,
// and the shell reads the terms itself (shell/shop/purchase).
//
// Kept per account in localStorage (a signed-out cart under its own key, taken into the account's once
// one signs in). Storage that throws or is missing leaves the cart in memory for the page's life.

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

const KEY_PREFIX = 'dcl-inworld-shop:cart:v1:'
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
  /** What a purchase bought: its lines go. */
  removeBought(items: readonly { contract: string; itemId: string }[]): void
  /** Whether one more unit of the item fits. */
  canAdd(item: Pick<InWorldCartLine, 'contract' | 'itemId' | 'available'>): boolean
}

export function createInWorldCart(opts: { storage?: () => CartStorage | null; now?: () => number } = {}): InWorldCart {
  const storage = opts.storage ?? defaultStorage
  const now = opts.now ?? Date.now
  const listeners = new Set<() => void>()
  let account: string | null = null
  let lines: readonly InWorldCartLine[] = []
  /** Every account's cart this page read or wrote, kept in memory when storage fails. */
  const memory = new Map<string, readonly InWorldCartLine[]>()

  const read = (who: string | null): readonly InWorldCartLine[] => {
    const key = keyOf(who)
    try {
      const raw = storage()?.getItem(key)
      if (raw != null) {
        const parsed = JSON.parse(raw) as unknown
        const list = Array.isArray(parsed) ? parsed.filter(isLine) : []
        // Each item once, within the cap, as this page writes it.
        const kept: InWorldCartLine[] = []
        for (const line of list) if (!kept.some((k) => sameItem(k, line)) && cartUnits(kept) + line.quantity <= MAX_PURCHASE_UNITS) kept.push(line)
        memory.set(key, kept)
        return kept
      }
    } catch {
      // Unreadable: what this page knows of it.
    }
    return memory.get(key) ?? []
  }
  const write = (who: string | null, next: readonly InWorldCartLine[]): void => {
    const key = keyOf(who)
    memory.set(key, next)
    try {
      const s = storage()
      if (next.length === 0) s?.removeItem(key)
      else s?.setItem(key, JSON.stringify(next))
    } catch {
      // Storage refused: the cart stays in memory.
    }
  }
  const commit = (next: readonly InWorldCartLine[]): void => {
    lines = next
    write(account, next)
    listeners.forEach((l) => l())
  }

  lines = read(null)

  const cart: InWorldCart = {
    getSnapshot: () => lines,
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
        const guest = read(null)
        if (guest.length > 0) {
          const merged = [...mine]
          for (const g of guest) {
            const at = merged.findIndex((l) => sameItem(l, g))
            const others = cartUnits(merged) - (at >= 0 ? merged[at].quantity : 0)
            const quantity = Math.min((at >= 0 ? merged[at].quantity : 0) + g.quantity, capFor(g, others))
            if (quantity <= 0) continue
            if (at >= 0) merged[at] = { ...merged[at], quantity }
            else merged.push({ ...g, quantity })
          }
          write(null, [])
          write(who, merged)
          mine = merged
        }
      }
      lines = mine
      listeners.forEach((l) => l())
    },
    canAdd(item) {
      const line = lines.find((l) => sameItem(l, item))
      const others = cartUnits(lines) - (line?.quantity ?? 0)
      return (line?.quantity ?? 0) + 1 <= capFor(line ?? item, others)
    },
    add(input) {
      const item = { ...input, contract: input.contract.toLowerCase() }
      if (!cart.canAdd(item)) return false
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
      if (lines.some((l) => sameItem(l, item))) commit(lines.filter((l) => !sameItem(l, item)))
    },
    removeBought(items) {
      if (lines.some((l) => items.some((i) => sameItem(l, i)))) commit(lines.filter((l) => !items.some((i) => sameItem(l, i))))
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
