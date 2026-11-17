// The Gift protocol between the HUD (app.html) and the web shell (/play), which holds the player's wallet. The HUD's
// picker names the token and the recipient; the shell checks both itself (the login's account owns the token on
// chain, the contract is a Decentraland collection, the recipient is another account), resolves the item and the
// recipient's profile on its own, and shows the transfer's progress, success and error over the app. Nothing that
// signs crosses from the HUD: parseGiftRequest admits the recipient and the token only.

import type { GiftRequest, PendingGift } from '@explorer-core/hostContract'

export const GIFT_PROTOCOL = 1

/** The methods the shell answers (`shellRequest`). */
export const GIFT_METHODS = {
  capabilities: 'gift.capabilities',
  /** The tokens sent as gifts that may not have left the account yet. */
  pending: 'gift.pending',
  open: 'gift.open'
} as const

/** The message the shell posts when a gift is sent. */
export const GIFT_EVENT = 'bevy-shell:gift'

/** The code `gift.open` answers with when this session may not gift here. */
export const GIFT_UNAVAILABLE = 'gift.unavailable'

/** Whether the signed-in session can gift here, and why not when it can't. */
export interface GiftCapabilities {
  v: typeof GIFT_PROTOCOL
  available: boolean
  /** Why Gift is hidden: the feature is off, a guest, or nobody is signed in. */
  reason?: 'disabled' | 'signedOut' | 'guest' | 'noWallet'
}

export interface GiftOpenRequest extends GiftRequest {
  v: typeof GIFT_PROTOCOL
  /** The token's `transferredAt` when it was picked, kept with the pending gift. */
  transferredAt: number
}

/** What the shell posts when a gift has been sent. */
export interface GiftEvent extends PendingGift {
  type: typeof GIFT_EVENT
  v: typeof GIFT_PROTOCOL
  state: 'sent'
}

const ADDRESS = /^0x[0-9a-fA-F]{40}$/
const TOKEN_ID = /^\d{1,78}$/
/** A collection item's token urn: `urn:decentraland:<network>:collections-v<n>:<contract>:<item>:<token>`. */
const TOKEN_URN = /^urn:decentraland:[a-z0-9]+:collections-v[12]:0x[0-9a-fA-F]{40}:[^:]{1,100}:\d{1,78}$/

const object = (v: unknown): Record<string, unknown> | null => (v != null && typeof v === 'object' && !Array.isArray(v) ? (v as Record<string, unknown>) : null)
const seconds = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v) && v >= 0

/** The HUD's gift request, or a thrown error naming what is wrong with it. Unknown fields are refused. */
export function parseGiftRequest(params: unknown): GiftOpenRequest {
  const p = object(params)
  if (!p) throw new Error('bad gift.open request')
  const extra = Object.keys(p).filter((k) => !['v', 'recipient', 'urn', 'tokenId', 'transferredAt'].includes(k))
  if (extra.length > 0) throw new Error(`unexpected gift.open fields: ${extra.join(', ')}`)
  if (p.v !== GIFT_PROTOCOL) throw new Error('unsupported gift protocol version')
  if (typeof p.recipient !== 'string' || !ADDRESS.test(p.recipient)) throw new Error('bad gift.open recipient')
  if (typeof p.tokenId !== 'string' || !TOKEN_ID.test(p.tokenId)) throw new Error('bad gift.open tokenId')
  if (typeof p.urn !== 'string' || !TOKEN_URN.test(p.urn) || !p.urn.endsWith(`:${p.tokenId}`)) throw new Error('bad gift.open urn')
  if (!seconds(p.transferredAt)) throw new Error('bad gift.open transferredAt')
  return { v: GIFT_PROTOCOL, recipient: p.recipient.toLowerCase(), urn: p.urn, tokenId: p.tokenId, transferredAt: p.transferredAt }
}

/** The shell's capabilities answer as the HUD reads it: anything it can't read is "unavailable". */
export function parseGiftCapabilities(value: unknown): GiftCapabilities {
  const p = object(value)
  if (!p || p.v !== GIFT_PROTOCOL || typeof p.available !== 'boolean') return { v: GIFT_PROTOCOL, available: false }
  return { v: GIFT_PROTOCOL, available: p.available }
}

/** A pending gift as stored or sent: its token urn and its `transferredAt` when it was sent. */
export function parsePendingGift(value: unknown): PendingGift | null {
  const p = object(value)
  if (!p || typeof p.urn !== 'string' || !TOKEN_URN.test(p.urn) || !seconds(p.transferredAt)) return null
  return { urn: p.urn, transferredAt: p.transferredAt }
}

/** The shell's pending answer as the HUD reads it: anything it can't read is dropped. */
export function parsePendingGifts(value: unknown): PendingGift[] {
  const p = object(value)
  if (!p || p.v !== GIFT_PROTOCOL || !Array.isArray(p.pending)) return []
  return p.pending.map(parsePendingGift).filter((g): g is PendingGift => g != null)
}

/** Whether a posted message is the shell's "gift sent". */
export function isGiftEvent(value: unknown): value is GiftEvent {
  const p = object(value)
  return p != null && p.type === GIFT_EVENT && p.v === GIFT_PROTOCOL && p.state === 'sent' && parsePendingGift(p) != null
}
