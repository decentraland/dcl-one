// The in-world Shop's purchase protocol between the HUD (app.html) and the web shell (/play), which holds
// the buyer's wallet. The HUD only says WHAT the buyer wants (an item, paid in Credits) and gets back an
// operation to follow; the shell reads the authoritative terms, asks the buyer to confirm in a dialog of
// its own, signs and submits. Nothing that decides a payment (calldata, typed data, the buyer, a spender,
// a relayer) crosses from the HUD: parseBuyRequest admits an item reference and a display hint only.

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

export interface ShopBuyRequest {
  v: typeof SHOP_PROTOCOL
  item: { contract: string; itemId: string }
  method: 'credits'
  /** The app's name for the creator. Not shown: the shell names the creator from their own profile. */
  creatorName?: string
  /** How BUY was pressed, so the dialog rings its first focus only for the keyboard. Cosmetic. */
  input?: 'pointer' | 'keyboard'
}

/**
 * reviewing: the dialog is open (loading, confirming, or short of credits); processing: signing or
 * waiting for the chain; pending: handed off and not confirmed yet; then succeeded, failed or cancelled.
 */
export type ShopOperationState = 'reviewing' | 'processing' | 'pending' | 'succeeded' | 'failed' | 'cancelled'

export const TERMINAL_STATES: readonly ShopOperationState[] = ['succeeded', 'failed', 'cancelled']

export interface ShopOperation {
  v: typeof SHOP_PROTOCOL
  operation: string
  state: ShopOperationState
  item: { contract: string; itemId: string }
  /** Where the buyer chose to go from the success dialog. */
  next?: 'backpack'
  txHash?: string | null
}

export type ShopOperationEvent = ShopOperation & { type: typeof SHOP_EVENT }

const ADDRESS = /^0x[0-9a-fA-F]{40}$/
/** A uint256 in decimal: at most 78 digits, no leading zeros. */
const UINT = /^(0|[1-9][0-9]{0,77})$/
const NAME_MAX = 64

function object(value: unknown): Record<string, unknown> | null {
  return value != null && typeof value === 'object' && !Array.isArray(value) ? (value as Record<string, unknown>) : null
}

/** The HUD's buy request, or a thrown error naming what is wrong with it. Unknown fields are refused. */
export function parseBuyRequest(params: unknown): ShopBuyRequest {
  const p = object(params)
  if (!p) throw new Error('bad shop.buy request')
  const extra = Object.keys(p).filter((k) => !['v', 'item', 'method', 'creatorName', 'input'].includes(k))
  if (extra.length > 0) throw new Error(`unexpected shop.buy fields: ${extra.join(', ')}`)
  if (p.v !== SHOP_PROTOCOL) throw new Error('unsupported shop protocol version')
  const item = object(p.item)
  if (!item || Object.keys(item).some((k) => k !== 'contract' && k !== 'itemId')) throw new Error('bad shop.buy item')
  const { contract, itemId } = item
  if (typeof contract !== 'string' || !ADDRESS.test(contract)) throw new Error('bad shop.buy contract')
  if (typeof itemId !== 'string' || !UINT.test(itemId)) throw new Error('bad shop.buy item id')
  if (p.method !== 'credits') throw new Error('unsupported shop.buy method')
  let creatorName: string | undefined
  if (p.creatorName !== undefined) {
    if (typeof p.creatorName !== 'string') throw new Error('bad shop.buy creator name')
    // Display text only: no control characters, and short.
    creatorName = p.creatorName.replace(/[\u0000-\u001f\u007f]/g, '').slice(0, NAME_MAX).trim() || undefined
  }
  if (p.input !== undefined && p.input !== 'pointer' && p.input !== 'keyboard') throw new Error('bad shop.buy input')
  const input = p.input as ShopBuyRequest['input']
  return { v: SHOP_PROTOCOL, item: { contract: contract.toLowerCase(), itemId }, method: 'credits', ...(creatorName ? { creatorName } : {}), ...(input ? { input } : {}) }
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
  const { operation, state, item, next, txHash } = message
  if (typeof operation !== 'string' || typeof state !== 'string' || !['reviewing', 'processing', 'pending', 'succeeded', 'failed', 'cancelled'].includes(state)) return null
  const it = object(item)
  if (!it || typeof it.contract !== 'string' || typeof it.itemId !== 'string') return null
  return {
    v: SHOP_PROTOCOL,
    operation,
    state: state as ShopOperationState,
    item: { contract: it.contract, itemId: it.itemId },
    ...(next === 'backpack' ? { next } : {}),
    ...(typeof txHash === 'string' || txHash === null ? { txHash } : {})
  }
}
