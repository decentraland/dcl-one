// The in-world Shop's purchase protocol between the HUD (app.html) and the web shell (/play), which holds
// the buyer's wallet. The HUD only says WHAT the buyer wants (an item, paid in Credits) and gets back an
// operation to follow; the shell reads the authoritative terms, asks the buyer to confirm in a dialog of
// its own, signs and submits. Nothing that decides a payment (calldata, typed data, the buyer, a spender,
// a relayer) crosses from the HUD: parseBuyRequest admits item references, how many of each, and display
// hints only. One item (BUY) and the cart's CHECKOUT are the same request: a list of one, or of the cart.
// A NAME (BUY NAME) is a request of its own: the NAME and the mode the shell said it buys NAMEs in, nothing
// else. Its operation is a variant of its own (the NAME, no items, no cart, and its mode), and every reader
// of operations (events and shop.status alike) goes through parseOperationEvent, which refuses a mix of the
// two. A HUD built before NAMEs reads `items: []` as no operation at all and drops it.

import { MAX_CREDIT_LINES_PER_GROUP } from '../../shop/model/settlement'
import { NAME_MAX_LENGTH, NAME_MIN_LENGTH } from '../../shop/names/names'
import type { WalletKind } from '../../shop/wallet/session'
import { ShellError } from './shellReply'

export const SHOP_PROTOCOL = 1

/** The methods the shell answers (`shellRequest`). */
export const SHOP_METHODS = {
  capabilities: 'shop.capabilities',
  balance: 'shop.balance',
  buy: 'shop.buy',
  buyName: 'shop.buyName',
  status: 'shop.status'
} as const

/**
 * The code `shop.buy` and `shop.buyName` answer with while another purchase is open, or still opening (its
 * dialog's first step shows before the request that opens it is answered): that purchase's dialog says what
 * is going on, so the HUD sends the buyer nowhere else. Matched by its code (isShellError), never by its message,
 * from a shell whose capabilities say `codedErrors`.
 */
export const SHOP_BUSY = 'shop.busy'

/**
 * The words of that answer, unchanged since before codes: HUDs from before them still match these words, and a
 * shell from before them (no `codedErrors`) answers with these words alone. A HUD can outlive its shell page's
 * version, and the reverse: the HUD reloads in its frame while the page stays (for the rest of the session, if
 * it is never reloaded). So the HUD takes these exact words as busy from such a shell only, and this adapter goes
 * when those shells can no longer host this HUD: with the next SHOP_PROTOCOL, whose answers this HUD alone reads
 * (a release by itself reloads no open tab's page).
 */
export const LEGACY_SHOP_BUSY_MESSAGE = 'a purchase is already open'

/** That answer, as the shell throws it. */
export const shopBusy = (): ShellError => new ShellError(SHOP_BUSY, LEGACY_SHOP_BUSY_MESSAGE)

/** The message the shell posts as an operation moves on. */
export const SHOP_EVENT = 'bevy-shell:shop'

/** The web Shop's own wallet kinds: a browser wallet, or an in-app (managed) one. */
export type ShopWalletKind = WalletKind

/**
 * How this shell buys a NAME. `simulation`: the whole flow is rehearsed (the real price, balance and
 * availability are read) and nothing is bought: no Credits are spent and the NAME is neither registered nor
 * reserved. `live`: it is registered. Absent: NAMEs aren't bought here, and the app sends the buyer to the
 * web Shop.
 */
export type ShopNamesMode = 'simulation' | 'live'
export const NAMES_MODES: readonly ShopNamesMode[] = ['simulation', 'live']

/** What the signed-in session can pay with here. `reason` says why it can't buy, when it can't. */
export interface ShopCapabilities {
  v: typeof SHOP_PROTOCOL
  /** The login's account (lowercase), null signed out. */
  buyer: string | null
  wallet: ShopWalletKind | null
  credits: boolean
  /** How the shell buys a NAME here (shop.buyName); absent where it doesn't. */
  namesMode?: ShopNamesMode
  reason?: 'signedOut' | 'guest' | 'noWallet'
  /**
   * Whether the login may use the Marketplace (resales, bids, NAME offers): a web3 wallet that is how the account
   * signed in, here to sign as it (shop/market/access.tsx). Absent means it may not.
   */
  marketplace?: boolean
  /**
   * Whether this shell's refusals carry a code (lib/shellReply), so the HUD tells a busy shell by SHOP_BUSY alone.
   * Absent from a shell from before codes, which says busy in LEGACY_SHOP_BUSY_MESSAGE's words only.
   */
  codedErrors?: boolean
}

export interface ShopBalance {
  v: typeof SHOP_PROTOCOL
  /** Whole credits. */
  credits: number
}

/** An item, and how many of it (a wearable or an emote can be minted more than once). */
export interface ShopLine {
  contract: string
  itemId: string
  quantity: number
}

/** The most a purchase buys at once: the web Shop's cap on a Credits group (one credit pays them all). */
export const MAX_PURCHASE_UNITS = MAX_CREDIT_LINES_PER_GROUP

export interface ShopBuyRequest {
  v: typeof SHOP_PROTOCOL
  /** Each item once, in the order to show them; at most MAX_PURCHASE_UNITS units in all. */
  items: ShopLine[]
  method: 'credits'
  /** Bought from the cart: its operations say so, for the app to take what they bought out of it. */
  cart?: true
  /** How BUY was pressed, so the dialog rings its first focus only for the keyboard. Cosmetic. */
  input?: 'pointer' | 'keyboard'
}

/** BUY NAME: the NAME to register, paid in Credits. */
export interface ShopBuyNameRequest {
  v: typeof SHOP_PROTOCOL
  /** 2 to 15 ASCII letters or digits, as typed. */
  name: string
  method: 'credits'
  /** The mode the shell's capabilities said. It decides nothing: the shell refuses a request whose mode isn't its own. */
  mode: ShopNamesMode
  /** How CLAIM NAME was pressed. Cosmetic. */
  input?: 'pointer' | 'keyboard'
}

/**
 * reviewing: the dialog is open (loading, confirming, or short of credits); processing: signing or
 * waiting for the chain; pending: handed off and not confirmed yet; then succeeded, failed or cancelled.
 */
export const OPERATION_STATES = ['reviewing', 'processing', 'pending', 'succeeded', 'failed', 'cancelled'] as const
export type ShopOperationState = (typeof OPERATION_STATES)[number]

export type ShopTerminalState = 'succeeded' | 'failed' | 'cancelled'
export const TERMINAL_STATES: readonly ShopOperationState[] = ['succeeded', 'failed', 'cancelled']
export const isTerminal = (state: ShopOperationState): state is ShopTerminalState => TERMINAL_STATES.includes(state)

interface ShopOperationBase {
  v: typeof SHOP_PROTOCOL
  operation: string
  /** Whose purchase it is (lowercase), as the shell holds it; null before anyone signed in. */
  buyer: string | null
  state: ShopOperationState
  txHash?: string | null
}

/** Items bought with Credits. */
export interface ShopItemOperation extends ShopOperationBase {
  /** What it buys: at least one. A request's items that settle in separate transactions are separate operations. */
  items: ShopLine[]
  /** Bought from the cart. */
  cart?: true
  /** The buyer left its success dialog for the Backpack. */
  next?: 'backpack'
  name?: undefined
  mode?: undefined
}

/** A NAME, bought (or, in `simulation`, rehearsed) with Credits. */
export interface ShopNameOperation extends ShopOperationBase {
  /** None: a NAME's operation buys no items. */
  items: []
  name: string
  mode: ShopNamesMode
  /**
   * What the buyer chose from its success dialog: a registered NAME (`live`) to wear as the avatar's name
   * (`assignName`), or a rehearsed one (`simulation`) to see over the avatar here, saved nowhere (`previewName`).
   */
  next?: 'assignName' | 'previewName'
  cart?: undefined
}

export type ShopOperation = ShopItemOperation | ShopNameOperation

export type ShopOperationEvent = ShopOperation & { type: typeof SHOP_EVENT }

/** An EVM address, in any case. */
export const ADDRESS = /^0x[0-9a-fA-F]{40}$/
/** A NAME exactly as registered: 2 to 15 ASCII letters or digits, nothing around them. */
export const NAME = new RegExp(`^[A-Za-z0-9]{${NAME_MIN_LENGTH},${NAME_MAX_LENGTH}}$`)
/** A uint256 in decimal: at most 78 digits, no leading zeros. */
export const UINT = /^(0|[1-9][0-9]{0,77})$/

/** Whether two references name the same item (contracts as given: lowercase, as parsed). */
export const sameItem = (a: { contract: string; itemId: string }, b: { contract: string; itemId: string }): boolean => a.contract === b.contract && a.itemId === b.itemId
/** How many units lines hold in all. */
export const unitsOf = (lines: readonly { quantity: number }[]): number => lines.reduce((n, l) => n + l.quantity, 0)

function object(value: unknown): Record<string, unknown> | null {
  return value != null && typeof value === 'object' && !Array.isArray(value) ? (value as Record<string, unknown>) : null
}

/** One `{ contract, itemId, quantity }`, or null; the contract lowercased. */
function parseLine(value: unknown): ShopLine | null {
  const line = object(value)
  if (!line || Object.keys(line).some((k) => k !== 'contract' && k !== 'itemId' && k !== 'quantity')) return null
  const { contract, itemId, quantity } = line
  if (typeof contract !== 'string' || !ADDRESS.test(contract) || typeof itemId !== 'string' || !UINT.test(itemId)) return null
  if (typeof quantity !== 'number' || !Number.isSafeInteger(quantity) || quantity < 1 || quantity > MAX_PURCHASE_UNITS) return null
  return { contract: contract.toLowerCase(), itemId, quantity }
}

/** A list of lines: at least one, each item once, at most MAX_PURCHASE_UNITS units in all; or null. */
export function parseLines(value: unknown): ShopLine[] | null {
  if (!Array.isArray(value) || value.length === 0 || value.length > MAX_PURCHASE_UNITS) return null
  const lines: ShopLine[] = []
  for (const v of value) {
    const line = parseLine(v)
    if (!line || lines.some((l) => l.contract === line.contract && l.itemId === line.itemId)) return null
    lines.push(line)
  }
  return lines.reduce((n, l) => n + l.quantity, 0) <= MAX_PURCHASE_UNITS ? lines : null
}

/** The HUD's buy request, or a thrown error naming what is wrong with it. Unknown fields are refused. */
export function parseBuyRequest(params: unknown): ShopBuyRequest {
  const p = object(params)
  if (!p) throw new Error('bad shop.buy request')
  const extra = Object.keys(p).filter((k) => !['v', 'items', 'method', 'cart', 'input'].includes(k))
  if (extra.length > 0) throw new Error(`unexpected shop.buy fields: ${extra.join(', ')}`)
  if (p.v !== SHOP_PROTOCOL) throw new Error('unsupported shop protocol version')
  const items = parseLines(p.items)
  if (!items) throw new Error('bad shop.buy items')
  if (p.method !== 'credits') throw new Error('unsupported shop.buy method')
  if (p.cart !== undefined && p.cart !== true) throw new Error('bad shop.buy cart')
  if (p.input !== undefined && p.input !== 'pointer' && p.input !== 'keyboard') throw new Error('bad shop.buy input')
  const input = p.input as ShopBuyRequest['input']
  return { v: SHOP_PROTOCOL, items, method: 'credits', ...(p.cart ? { cart: true as const } : {}), ...(input ? { input } : {}) }
}

const isNamesMode = (value: unknown): value is ShopNamesMode => (NAMES_MODES as readonly unknown[]).includes(value)

/**
 * The HUD's BUY NAME request, or a thrown error naming what is wrong with it. Unknown fields are refused,
 * and nothing in it decides the purchase: the price, the buyer and what is called are the shell's own.
 */
export function parseBuyNameRequest(params: unknown): ShopBuyNameRequest {
  const p = object(params)
  if (!p) throw new Error('bad shop.buyName request')
  const extra = Object.keys(p).filter((k) => !['v', 'name', 'method', 'mode', 'input'].includes(k))
  if (extra.length > 0) throw new Error(`unexpected shop.buyName fields: ${extra.join(', ')}`)
  if (p.v !== SHOP_PROTOCOL) throw new Error('unsupported shop protocol version')
  if (typeof p.name !== 'string' || !NAME.test(p.name)) throw new Error('bad shop.buyName name')
  if (p.method !== 'credits') throw new Error('unsupported shop.buyName method')
  if (!isNamesMode(p.mode)) throw new Error('bad shop.buyName mode')
  if (p.input !== undefined && p.input !== 'pointer' && p.input !== 'keyboard') throw new Error('bad shop.buyName input')
  const input = p.input as ShopBuyNameRequest['input']
  return { v: SHOP_PROTOCOL, name: p.name, method: 'credits', mode: p.mode, ...(input ? { input } : {}) }
}

/** `{ operation }` for shop.status. */
export function parseOperationRef(params: unknown): string {
  const p = object(params)
  const id = p?.operation
  if (typeof id !== 'string' || !/^[0-9a-f-]{8,64}$/.test(id)) throw new Error('bad shop.status request')
  return id
}

/**
 * A shell event (or one of shop.status's operations, given the event's `type`) the HUD can trust the shape
 * of; the source and origin are checked by onShellMessage. Exactly one of two variants, or null:
 * - items: at least one line, no `name` or `mode`, `cart` only as true, and `next: 'backpack'` only once
 *   it succeeded;
 * - a NAME: a valid NAME, `items: []`, no `cart`, an explicit mode, no transaction for a simulation, and
 *   `next` only once it succeeded: `assignName` for a live NAME, `previewName` for a simulated one.
 */
export function parseOperationEvent(message: Record<string, unknown>): ShopOperation | null {
  if (message.type !== SHOP_EVENT || message.v !== SHOP_PROTOCOL) return null
  const { operation, buyer, state, items, name, mode, cart, next, txHash } = message
  if (typeof operation !== 'string' || typeof state !== 'string' || !(OPERATION_STATES as readonly string[]).includes(state)) return null
  if (buyer !== null && (typeof buyer !== 'string' || !ADDRESS.test(buyer))) return null
  if (txHash !== undefined && txHash !== null && typeof txHash !== 'string') return null
  if (cart !== undefined && cart !== true) return null
  const base: ShopOperationBase = {
    v: SHOP_PROTOCOL,
    operation,
    buyer: buyer === null ? null : buyer.toLowerCase(),
    state: state as ShopOperationState,
    ...(txHash !== undefined ? { txHash: txHash as string | null } : {})
  }
  const succeeded = state === 'succeeded'
  if (name === undefined) {
    if (mode !== undefined) return null
    const lines = parseLines(items)
    if (!lines) return null
    if (next !== undefined && !(next === 'backpack' && succeeded)) return null
    return { ...base, items: lines, ...(cart ? { cart: true as const } : {}), ...(next ? { next: 'backpack' as const } : {}) }
  }
  if (typeof name !== 'string' || !NAME.test(name) || !Array.isArray(items) || items.length !== 0 || cart !== undefined || !isNamesMode(mode)) return null
  if (mode === 'simulation' && typeof txHash === 'string') return null
  if (next !== undefined && !(succeeded && ((mode === 'live' && next === 'assignName') || (mode === 'simulation' && next === 'previewName')))) return null
  return { ...base, items: [], name, mode, ...(next ? { next: next as ShopNameOperation['next'] } : {}) }
}
