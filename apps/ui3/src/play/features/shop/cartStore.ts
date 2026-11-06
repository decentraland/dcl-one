// The in-world Shop's cart (the design's Cart flow): what the buyer added with ADD TO CART and how many of
// each, in this browser. It is its own, apart from the web Shop's cart (another design, another store).
// Each line keeps what identifies the item and a snapshot to draw it with (its name, art and the card's
// price when it was added), never a price authority: CHECKOUT sends only the items and how many of each,
// and the shell reads the terms itself (shell/shop/purchase).
//
// Kept per account in localStorage (a signed-out cart under its own key, taken into the account's once
// one signs in, as a transfer with an id of its own that the account records: taken once, however often
// it is tried again or whichever tab takes it; see `adopt`).
//
// What a purchase from the cart bought leaves the cart of the account that bought it (whichever is active
// now): as many of each item as it bought, once per operation, however often its success is heard (an
// event, the shell's status after a reload). The cart and the operations already taken out of it are one
// record per account, written whole in one go: a write that sticks keeps both, one that doesn't keeps
// neither, so a replay after a reload takes the purchase out exactly once either way.
//
// Every change, in every tab, is one read-change-write of the record under one Web Lock (`mutate`): read as
// storage holds it then, so no tab writes over another tab's change. A change storage doesn't take (full,
// blocked) stays this page's, replayed on whatever storage holds next and written with the next change it
// takes; it is never written back as a stale whole. Another tab's write is shown as it lands (the
// `storage` event). The scope: Web Locks order the writes of the origin's tabs; how localStorage itself is
// shared between them is the browser's (the HTML standard leaves it undefined across agent clusters).
// Without Web Locks, only this page's own changes go one at a time.

import { useCallback, useEffect, useSyncExternalStore } from 'react'
import { cardPrice } from '../../../shop/discovery/cardPrice'
import type { CatalogItem } from '../../../shop/discovery/lib/catalogItem'
import { SIGNED_OUT } from '../../../shop/model/settlement'
import { ADDRESS, MAX_PURCHASE_UNITS, sameItem, UINT, unitsOf, type ShopLine } from '../../lib/shopProtocol'

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

const keyOf = (account: string | null): string => KEY_PREFIX + (account ?? GUEST)
/** An address as this page keeps them: lowercase. */
const isAddress = (s: string): boolean => ADDRESS.test(s) && s === s.toLowerCase()
/** How many units the cart holds. */
export const cartUnits = unitsOf

function isLine(v: unknown): v is InWorldCartLine {
  if (v == null || typeof v !== 'object' || Array.isArray(v)) return false
  const l = v as Record<string, unknown>
  return (
    typeof l.contract === 'string' &&
    isAddress(l.contract) &&
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

/** Units of an item a signed-out cart's transfer moved into an account's. */
interface Taken {
  contract: string
  itemId: string
  units: number
}

/**
 * An account's cart: its lines, the operations whose purchase already left it, and the signed-out carts'
 * transfers it already took (by the transfer's id, with what it took). A signed-out cart carries its
 * transfer under way, its id and the account it goes to, written before that account takes it: tried
 * again, from whichever login, it is settled against that account, so it is taken once and by it alone.
 */
interface CartRecord {
  lines: readonly InWorldCartLine[]
  applied: readonly string[]
  adopted: readonly { id: string; taken: readonly Taken[] }[]
  transfer?: { id: string; to: string }
  /** On a signed-out cart: what an account's cap left over from taking it, already offered to that account. */
  held?: string
}
const EMPTY: CartRecord = { lines: [], applied: [], adopted: [] }
/** Transfers remembered as taken (newest kept). */
const MAX_ADOPTED = 20
const isTaken = (t: unknown): t is Taken =>
  t != null && typeof t === 'object' && typeof (t as Taken).contract === 'string' && isAddress((t as Taken).contract) && typeof (t as Taken).itemId === 'string' && UINT.test((t as Taken).itemId) && Number.isSafeInteger((t as Taken).units) && (t as Taken).units > 0

/** A stored record in the shape this page writes, its lines each item once and within the cap; or null. */
function parseRecord(raw: string): CartRecord | null {
  const v = JSON.parse(raw) as unknown
  if (v == null || typeof v !== 'object' || Array.isArray(v)) return null
  const r = v as Record<string, unknown>
  if (r.v !== RECORD_VERSION || !Array.isArray(r.lines) || !Array.isArray(r.applied)) return null
  const lines: InWorldCartLine[] = []
  for (const line of r.lines.filter(isLine)) if (!lines.some((k) => sameItem(k, line)) && cartUnits(lines) + line.quantity <= MAX_PURCHASE_UNITS) lines.push(line)
  const applied = r.applied.filter((id): id is string => typeof id === 'string' && OPERATION_ID.test(id)).slice(-MAX_APPLIED)
  const adopted = (Array.isArray(r.adopted) ? r.adopted : [])
    .filter((a): a is { id: string; taken: Taken[] } => a != null && typeof a === 'object' && typeof a.id === 'string' && OPERATION_ID.test(a.id) && Array.isArray(a.taken) && a.taken.every(isTaken))
    .slice(-MAX_ADOPTED)
  const t = r.transfer as Record<string, unknown> | undefined
  const transfer = t != null && typeof t === 'object' && typeof t.id === 'string' && OPERATION_ID.test(t.id) && typeof t.to === 'string' && isAddress(t.to) ? { id: t.id, to: t.to } : undefined
  const held = typeof r.held === 'string' && isAddress(r.held) ? r.held : undefined
  return { lines, applied, adopted, ...(transfer ? { transfer } : {}), ...(held ? { held } : {}) }
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
  add(input: InWorldCartInput): Promise<boolean>
  setQuantity(item: { contract: string; itemId: string }, quantity: number): Promise<void>
  remove(item: { contract: string; itemId: string }): Promise<void>
  /**
   * What `operation`, a purchase from the cart by `buyer`, bought leaves that account's cart: as many of
   * each item as it bought. Once per operation; a repeat changes nothing.
   */
  applyPurchase(buyer: string, operation: string, items: readonly ShopLine[]): Promise<void>
  /** Whether one more unit of the item fits, against `item.available` (the stock as the caller reads it now). */
  canAdd(item: Pick<InWorldCartLine, 'contract' | 'itemId' | 'available'>): boolean
}

/** Calls back with the storage key another tab wrote (null: it cleared storage); returns the unsubscribe. */
export type StorageWatch = (onChange: (key: string | null) => void) => () => void

const windowStorageEvents: StorageWatch = (onChange) => {
  if (typeof window === 'undefined') return () => {}
  const listener = (e: StorageEvent): void => onChange(e.key)
  window.addEventListener('storage', listener)
  return () => window.removeEventListener('storage', listener)
}

/** Runs `fn` holding the cart's lock, which every tab's cart writes take; resolves with what it returns. */
export type CartLock = <T>(fn: () => T) => Promise<T>

const LOCK = 'dcl-inworld-shop:cart'

/** Web Locks across the origin's tabs; without them, this page's own writes still go one at a time. */
function cartLock(): CartLock {
  let chain: Promise<unknown> = Promise.resolve()
  const local: CartLock = (fn) => {
    const run = chain.then(fn)
    chain = run.catch(() => {})
    return run
  }
  return <T,>(fn: () => T): Promise<T> => {
    const locks = typeof navigator !== 'undefined' ? navigator.locks : undefined
    // `fn` is synchronous: the lock is held from the read to the write, and goes as it returns.
    return locks ? (locks.request(LOCK, () => fn()) as Promise<T>) : local(fn)
  }
}

export function createInWorldCart(opts: { storage?: () => CartStorage | null; now?: () => number; watch?: StorageWatch; lock?: CartLock; newId?: () => string } = {}): InWorldCart {
  const storage = opts.storage ?? defaultStorage
  const newId = opts.newId ?? (() => crypto.randomUUID())
  const now = opts.now ?? Date.now
  const lock = opts.lock ?? cartLock()
  const listeners = new Set<() => void>()
  let account: string | null = null
  /** The active account's record; its lines are the snapshot. */
  let current: CartRecord = EMPTY
  type Change = (record: CartRecord) => CartRecord | null
  /** What storage last gave for each record (its text and the record parsed from it). */
  const stored = new Map<string, { raw: string | null; record: CartRecord }>()
  /** This page's changes to a record that storage didn't take yet, replayed on what storage holds. */
  const pending = new Map<string, Change[]>()
  /** The last view of each record: what storage held, with the very list of pending changes put on it. */
  const views = new Map<string, { base: CartRecord; changes: readonly Change[]; record: CartRecord }>()

  /** The record as storage holds it now; what it last gave when it can't be read. */
  const fromStorage = (key: string): CartRecord => {
    let raw: string | null
    try {
      const s = storage()
      if (!s) return stored.get(key)?.record ?? EMPTY
      raw = s.getItem(key)
    } catch {
      return stored.get(key)?.record ?? EMPTY
    }
    const last = stored.get(key)
    if (last && last.raw === raw) return last.record
    let record = EMPTY
    try {
      record = raw == null ? EMPTY : (parseRecord(raw) ?? EMPTY)
    } catch {
      // Not this page's shape: an empty cart.
    }
    stored.set(key, { raw, record })
    return record
  }
  /**
   * `who`'s record as this page sees it: what storage holds now (whichever tab wrote it), with this page's
   * changes that storage didn't take yet replayed on it. The same object while neither changes.
   */
  const read = (who: string | null): CartRecord => {
    const key = keyOf(who)
    const base = fromStorage(key)
    const changes = pending.get(key)
    if (!changes || changes.length === 0) return base
    // The same base and the same list (each change makes a new one), or it is put together again.
    const view = views.get(key)
    if (view && view.base === base && view.changes === changes) return view.record
    const record = changes.reduce((r, change) => change(r) ?? r, base)
    views.set(key, { base, changes, record })
    return record
  }
  /** Writes the whole record in one go; false when storage didn't take it. */
  const persist = (key: string, next: CartRecord): boolean => {
    try {
      const s = storage()
      if (!s) return false
      if (next.lines.length === 0 && next.applied.length === 0 && next.adopted.length === 0) {
        s.removeItem(key)
        stored.set(key, { raw: null, record: EMPTY })
      } else {
        const raw = JSON.stringify({ v: RECORD_VERSION, lines: next.lines, applied: next.applied, adopted: next.adopted, ...(next.transfer ? { transfer: next.transfer } : {}), ...(next.held ? { held: next.held } : {}) })
        s.setItem(key, raw)
        stored.set(key, { raw, record: next })
      }
      return true
    } catch {
      return false
    }
  }
  /**
   * `next`, derived from `who`'s record as it stands (its pending changes on it), written whole; storage
   * taking it takes the pending changes with it. False when storage didn't.
   */
  const commitRecord = (who: string | null, next: CartRecord): boolean => {
    const key = keyOf(who)
    views.delete(key)
    if (!persist(key, next)) return false
    pending.delete(key)
    return true
  }
  /**
   * One change applied to `who`'s record as it stands (call it holding the lock): written whole if storage
   * takes it, which takes this page's pending changes with it; otherwise kept to replay on whatever
   * storage holds next, never written over another tab's record as a stale whole.
   */
  const apply = (who: string | null, change: Change): boolean => {
    const next = change(read(who))
    if (!next) return false
    if (!commitRecord(who, next)) pending.set(keyOf(who), [...(pending.get(keyOf(who)) ?? []), change])
    return true
  }

  /** Accounts whose sign-in transfer a refused write cut short: tried again after this account's next write. */
  const unfinished = new Set<string>()

  /**
   * The signed-out cart moved into `who`'s (call it holding the lock), as a transfer taken once and by one
   * account: the transfer (its id, and `who` as where it goes) is written on the signed-out cart first;
   * the account's cart takes its units within the cap and records the transfer, in one write; then the
   * signed-out cart lets go of the units taken. What didn't fit stays in the signed-out cart, held as
   * already offered to `who`, so it doesn't flow back into that cart as the buyer changes it; it is free
   * again for another account, or once the signed-out cart is changed. A transfer under way to another
   * account is settled against that one first: let go of if it took it, free for `who` if it never did. A
   * step storage refuses ends it and marks it unfinished, to be tried again after this account's next
   * write that lands (or the next sign-in); nothing of it waits in `pending`, so it is never added twice.
   */
  const adopt = (who: string): void => {
    unfinished.delete(who)
    const refused = (): void => void unfinished.add(who)
    /** The signed-out cart less what a transfer took, with no transfer under way; false when refused. */
    const letGo = (guest: CartRecord, taken: readonly Taken[], held?: string): boolean => {
      const lines = guest.lines.flatMap((l) => {
        const units = taken.find((t) => sameItem(t, l))?.units ?? 0
        return l.quantity > units ? [{ ...l, quantity: l.quantity - units }] : []
      })
      return commitRecord(null, { lines, applied: guest.applied, adopted: [], ...(held && lines.length > 0 ? { held } : {}) })
    }
    // A transfer found taken is let go of first; then what is left (added since, or that didn't fit) can go.
    for (let round = 0; round < 3; round++) {
      const guest = read(null)
      if (guest.lines.length === 0) return
      let transfer = guest.transfer
      // What is left over was already offered to this account, and didn't fit.
      if (!transfer && guest.held === who) return
      if (transfer && transfer.to !== who) {
        const theirs = read(transfer.to).adopted.find((a) => a.id === transfer!.id)
        if (theirs) {
          if (!letGo(guest, theirs.taken, transfer.to)) return refused()
          continue
        }
        // That account never took it: it is free to go to this one, as a transfer of its own.
        transfer = undefined
      }
      if (!transfer) {
        transfer = { id: newId(), to: who }
        if (!commitRecord(null, { ...guest, transfer, held: undefined })) return refused()
      }
      const mine = read(who)
      const done = mine.adopted.find((a) => a.id === transfer!.id)
      if (done) {
        if (!letGo(guest, done.taken, who)) return refused()
        continue
      }
      const merged = [...mine.lines]
      const took: Taken[] = []
      for (const g of guest.lines) {
        const at = merged.findIndex((l) => sameItem(l, g))
        const had = at >= 0 ? merged[at].quantity : 0
        // The merged line keeps the larger stock it knows of, so its own cap never cuts what it now holds.
        const available = at < 0 ? g.available : merged[at].available == null || g.available == null ? null : Math.max(merged[at].available!, g.available)
        const quantity = Math.max(had, Math.min(had + g.quantity, capFor({ available }, cartUnits(merged) - had)))
        if (quantity === had) continue
        took.push({ contract: g.contract, itemId: g.itemId, units: quantity - had })
        if (at >= 0) merged[at] = { ...merged[at], quantity, available }
        else merged.push({ ...g, quantity })
      }
      // Nothing fits: the signed-out cart waits, held as offered to this account.
      if (took.length === 0) {
        if (!letGo(guest, [], who)) refused()
        return
      }
      if (!commitRecord(who, { ...mine, lines: merged, adopted: [...mine.adopted, { id: transfer.id, taken: took }].slice(-MAX_ADOPTED) })) return refused()
      // Freshly taken: let go of it; what is left didn't fit, and is held.
      if (!letGo(guest, took, who)) refused()
      return
    }
  }

  const notify = (): void => listeners.forEach((l) => l())
  /** The active account's record as it stands now, shown. */
  const refresh = (): void => {
    const next = read(account)
    if (next === current) return
    current = next
    notify()
  }

  /**
   * One change to `who`'s record, whole, under the cart's lock: read as every tab's last write left it
   * (with this page's untaken changes on it), changed, and written before the lock goes, so no tab's
   * change is written over by another's. `change` answers null to leave the record as it is.
   */
  const mutate = (who: string | null, change: Change): Promise<boolean> =>
    lock(() => {
      // A change to the signed-out cart is the buyer's own: what it holds is free to go again.
      const changed = apply(who, who === null ? (r) => withoutHeld(change(r)) : change)
      // A sign-in's transfer that storage cut short is tried again once this account's cart is written.
      if (who !== null && who === account && unfinished.has(who) && !pending.has(keyOf(who))) adopt(who)
      if (who === account) refresh()
      return changed
    })

  /** A record as changed, free of what it held for an account (or null, unchanged). */
  const withoutHeld = (record: CartRecord | null): CartRecord | null => {
    if (!record?.held) return record
    const { held: _held, ...rest } = record
    return rest
  }

  /** Lines with a line's quantity set, within its cap, or null when that changes nothing. */
  const withQuantity = (record: CartRecord, item: { contract: string; itemId: string }, quantity: number): CartRecord | null => {
    const at = record.lines.findIndex((l) => sameItem(l, item))
    if (at < 0) return null
    const line = record.lines[at]
    const q = Math.min(Math.max(1, Math.floor(quantity)), capFor(line, cartUnits(record.lines) - line.quantity))
    if (q < 1 || q === line.quantity) return null
    const lines = [...record.lines]
    lines[at] = { ...line, quantity: q }
    return { ...record, lines }
  }

  current = read(null)

  // Another tab wrote: the active account's record is shown again as it stands (with this page's untaken
  // changes on it); any other is read when next needed.
  ;(opts.watch ?? windowStorageEvents)((key) => {
    if (key !== null && !key.startsWith(KEY_PREFIX)) return
    if (key === null || key === keyOf(account)) refresh()
  })

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
      current = read(who)
      notify()
      if (who == null) return
      // A signed-out cart is the account's once it signs in: its lines join, within the cap, under the lock.
      void lock(() => {
        adopt(who)
        if (account === who) refresh()
      })
    },
    canAdd(item) {
      const { lines } = read(account)
      const line = lines.find((l) => sameItem(l, item))
      const others = cartUnits(lines) - (line?.quantity ?? 0)
      return (line?.quantity ?? 0) + 1 <= capFor(item, others)
    },
    add(input) {
      const item = { ...input, contract: input.contract.toLowerCase() }
      return mutate(account, (record) => {
        const at = record.lines.findIndex((l) => sameItem(l, item))
        const others = cartUnits(record.lines) - (at >= 0 ? record.lines[at].quantity : 0)
        if ((at >= 0 ? record.lines[at].quantity : 0) + 1 > capFor(item, others)) return null
        if (at < 0) return { ...record, lines: [...record.lines, { ...item, quantity: 1, addedAt: now() }] }
        const lines = [...record.lines]
        // The newer snapshot (its price and stock as the listing reads them now), one more of it.
        lines[at] = { ...record.lines[at], ...item, quantity: record.lines[at].quantity + 1 }
        return { ...record, lines }
      })
    },
    async setQuantity(item, quantity) {
      await mutate(account, (record) => withQuantity(record, item, quantity))
    },
    async remove(item) {
      await mutate(account, (record) => (record.lines.some((l) => sameItem(l, item)) ? { ...record, lines: record.lines.filter((l) => !sameItem(l, item)) } : null))
    },
    async applyPurchase(buyer, operation, items) {
      // What it bought and that it was taken out, together: one record, one write, under the lock.
      await mutate(buyer.toLowerCase(), (record) =>
        record.applied.includes(operation)
          ? null
          : {
              ...record,
              lines: record.lines.flatMap((l) => {
                const bought = items.reduce((n, i) => (sameItem(l, i) ? n + i.quantity : n), 0)
                if (bought === 0) return [l]
                return l.quantity > bought ? [{ ...l, quantity: l.quantity - bought }] : []
              }),
              applied: [...record.applied, operation].slice(-MAX_APPLIED)
            }
      )
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
export function inWorldCartInputFor(item: CatalogItem): InWorldCartInput | null {
  const price = cardPrice(item, SIGNED_OUT)
  if (price.kind !== 'credits' || item.itemId == null || !isAddress(item.contractAddress.toLowerCase())) return null
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
