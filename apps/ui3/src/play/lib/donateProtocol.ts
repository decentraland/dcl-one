// The Donate protocol between the HUD (app.html) and the web shell (/play), which holds the player's wallet. The
// HUD only names WHICH place to tip (a parcel of Genesis City, or a World); the shell resolves that place's
// creator itself, shows the player the panel with the creator, their balance and the amount, signs and sends.
// Nothing that decides the payment (the recipient, the amount, the token, the chain) crosses from the HUD:
// parseDonateRequest admits a place reference and a display hint only.

import type { DonatePlace } from '@explorer-core/hostContract'
import { ShellError } from '@explorer-core/lib/shellReply'

export const DONATE_PROTOCOL = 1

/** The least a tip may be, in MANA: the catalyst tells a creator of no smaller tip. */
export const MIN_TIP = 1

/** A tip amount as typed: whole MANA, optionally with up to 18 decimals (MANA's precision; `10` or `10.5`, not `10.`
 *  or `.5`). The panel and the
 *  shell read amounts by this one rule. */
export const TIP_AMOUNT = /^\d+(\.\d{1,18})?$/

/** The methods the shell answers (`shellRequest`). */
export const DONATE_METHODS = {
  capabilities: 'donate.capabilities',
  /** Whether a place has a creator to tip (the shell reads it): Donate shows only where it does. */
  target: 'donate.target',
  open: 'donate.open'
} as const

/** The message the shell posts when a tip is sent, for the app to play the tip emote. */
export const DONATE_EVENT = 'bevy-shell:donate'

/** The code `donate.open` answers with while the shell's dialog is up for something else. */
export const DONATE_BUSY = 'donate.busy'
export const donateBusy = (): ShellError => new ShellError(DONATE_BUSY, 'a dialog is already open')

/** The code `donate.open` answers with when this session may not tip here. */
export const DONATE_UNAVAILABLE = 'donate.unavailable'

/** Whether the signed-in session can tip here, and why not when it can't. */
export interface DonateCapabilities {
  v: typeof DONATE_PROTOCOL
  available: boolean
  /** Why Donate is hidden: the feature is off, the session isn't a web3 wallet sign-in, or nobody is signed in. */
  reason?: 'disabled' | 'signedOut' | 'notWeb3'
}

/** A place to tip: a parcel of Genesis City (any of the place's parcels), or a World by its name. */
export type { DonatePlace }

export interface DonateRequest {
  v: typeof DONATE_PROTOCOL
  place: DonatePlace
  /** How Donate was pressed, so the panel rings its first focus only for the keyboard. Cosmetic. */
  input?: 'pointer' | 'keyboard'
}

/** What the shell posts when a tip has been sent. */
export interface DonateEvent {
  type: typeof DONATE_EVENT
  v: typeof DONATE_PROTOCOL
  state: 'sent'
}

const PARCEL_LIMIT = 1000
/** A World's name: a NAME (or an ENS name) ending in `.eth`, as the realm names it. */
const WORLD = /^[a-z0-9][a-z0-9-]{0,62}(\.[a-z0-9][a-z0-9-]{0,62})*\.eth$/

const object = (v: unknown): Record<string, unknown> | null => (v != null && typeof v === 'object' && !Array.isArray(v) ? (v as Record<string, unknown>) : null)
const coordinate = (v: unknown): v is number => typeof v === 'number' && Number.isInteger(v) && Math.abs(v) <= PARCEL_LIMIT

function parsePlace(value: unknown): DonatePlace {
  const p = object(value)
  if (!p) throw new Error('bad donate.open place')
  const keys = Object.keys(p)
  if (keys.length !== 1) throw new Error('bad donate.open place')
  if ('world' in p) {
    const world = typeof p.world === 'string' ? p.world.toLowerCase() : ''
    if (!WORLD.test(world)) throw new Error('bad donate.open world')
    return { world }
  }
  const parcel = object(p.parcel)
  if (!parcel || Object.keys(parcel).length !== 2 || !coordinate(parcel.x) || !coordinate(parcel.y)) throw new Error('bad donate.open parcel')
  return { parcel: { x: parcel.x, y: parcel.y } }
}

/** `{ v, place }` for donate.target: the place to look up, nothing else. */
export function parseDonateTarget(params: unknown): DonatePlace {
  const p = object(params)
  if (!p || Object.keys(p).some((k) => k !== 'v' && k !== 'place') || p.v !== DONATE_PROTOCOL) throw new Error('bad donate.target request')
  return parsePlace(p.place)
}

/** The HUD's Donate request, or a thrown error naming what is wrong with it. Unknown fields are refused. */
export function parseDonateRequest(params: unknown): DonateRequest {
  const p = object(params)
  if (!p) throw new Error('bad donate.open request')
  const extra = Object.keys(p).filter((k) => !['v', 'place', 'input'].includes(k))
  if (extra.length > 0) throw new Error(`unexpected donate.open fields: ${extra.join(', ')}`)
  if (p.v !== DONATE_PROTOCOL) throw new Error('unsupported donate protocol version')
  const place = parsePlace(p.place)
  if (p.input !== undefined && p.input !== 'pointer' && p.input !== 'keyboard') throw new Error('bad donate.open input')
  const input = p.input as DonateRequest['input']
  return { v: DONATE_PROTOCOL, place, ...(input ? { input } : {}) }
}

/** The shell's capabilities answer as the HUD reads it: anything it can't read is "unavailable". */
export function parseDonateCapabilities(value: unknown): DonateCapabilities {
  const p = object(value)
  if (!p || p.v !== DONATE_PROTOCOL || typeof p.available !== 'boolean') return { v: DONATE_PROTOCOL, available: false }
  return { v: DONATE_PROTOCOL, available: p.available }
}

/** Whether a posted message is the shell's "tip sent". */
export function isDonateEvent(value: unknown): value is DonateEvent {
  const p = object(value)
  return p != null && p.type === DONATE_EVENT && p.v === DONATE_PROTOCOL && p.state === 'sent'
}
