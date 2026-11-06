// The in-world Shop's purchase protocol between the HUD (app.html) and the web shell (/play), which holds
// the buyer's wallet. The HUD only says WHAT the buyer wants (an item, paid in Credits) and gets back an
// operation to follow; the shell reads the authoritative terms, asks the buyer to confirm in a dialog of
// its own, signs and submits. Nothing that decides a payment (calldata, typed data, the buyer, a spender,
// a relayer) crosses from the HUD: parseBuyRequest admits item references, how many of each, and display
// hints only. One item (BUY) and the cart's CHECKOUT are the same request: a list of one, or of the cart.

import { MAX_CREDIT_LINES_PER_GROUP } from '../../shop/model/settlement'
import type { WalletKind } from '../../shop/wallet/session'

export const SHOP_PROTOCOL = 1

/** The methods the shell answers (`shellRequest`). */
export const SHOP_METHODS = {
  capabilities: 'shop.capabilities',
  balance: 'shop.balance',
  buy: 'shop.buy',
  status: 'shop.status'
} as const

/** The message the shell posts as an operation moves on. */
export const SHOP_EVENT = 'bevy-shell:shop'

/** The web Shop's own wallet kinds: a browser wallet, or an in-app (managed) one. */
export type ShopWalletKind = WalletKind

/** What the signed-in session can pay with here. `reason` says why it can't buy, when it can't. */
export interface ShopCapabilities {
  v: typeof SHOP_PROTOCOL
  /** The login's account (lowercase), null signed out. */
  buyer: string | null
  wallet: ShopWalletKind | null
  credits: boolean
  reason?: 'signedOut' | 'guest' | 'noWallet'
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

/**
 * reviewing: the dialog is open (loading, confirming, or short of credits); processing: signing or
 * waiting for the chain; pending: handed off and not confirmed yet; then succeeded, failed or cancelled.
 */
export const OPERATION_STATES = ['reviewing', 'processing', 'pending', 'succeeded', 'failed', 'cancelled'] as const
export type ShopOperationState = (typeof OPERATION_STATES)[number]

export type ShopTerminalState = 'succeeded' | 'failed' | 'cancelled'
export const TERMINAL_STATES: readonly ShopOperationState[] = ['succeeded', 'failed', 'cancelled']
export const isTerminal = (state: ShopOperationState): state is ShopTerminalState => TERMINAL_STATES.includes(state)

export interface ShopOperation {
  v: typeof SHOP_PROTOCOL
  operation: string
  /** Whose purchase it is (lowercase), as the shell holds it; null before anyone signed in. */
  buyer: string | null
  state: ShopOperationState
  /** What it buys. A request's items that settle in separate transactions are separate operations. */
  items: ShopLine[]
  /** Bought from the cart. */
  cart?: true
  /** Where the buyer chose to go from the success dialog. */
  next?: 'backpack'
  txHash?: string | null
}

export type ShopOperationEvent = ShopOperation & { type: typeof SHOP_EVENT }

/** An EVM address, in any case. */
export const ADDRESS = /^0x[0-9a-fA-F]{40}$/
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

/** `{ operation }` for shop.status. */
export function parseOperationRef(params: unknown): string {
  const p = object(params)
  const id = p?.operation
  if (typeof id !== 'string' || !/^[0-9a-f-]{8,64}$/.test(id)) throw new Error('bad shop.status request')
  return id
}

/** A shell event the HUD can trust the shape of (the source and origin are checked by onShellMessage). */
export function parseOperationEvent(message: Record<string, unknown>): ShopOperation | null {
  if (message.type !== SHOP_EVENT || message.v !== SHOP_PROTOCOL) return null
  const { operation, buyer, state, items, cart, next, txHash } = message
  if (typeof operation !== 'string' || typeof state !== 'string' || !(OPERATION_STATES as readonly string[]).includes(state)) return null
  if (buyer !== null && (typeof buyer !== 'string' || !ADDRESS.test(buyer))) return null
  const lines = parseLines(items)
  if (!lines) return null
  return {
    v: SHOP_PROTOCOL,
    operation,
    buyer: buyer === null ? null : buyer.toLowerCase(),
    state: state as ShopOperationState,
    items: lines,
    ...(cart === true ? { cart } : {}),
    ...(next === 'backpack' ? { next } : {}),
    ...(typeof txHash === 'string' || txHash === null ? { txHash } : {})
  }
}
