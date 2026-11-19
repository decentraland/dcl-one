// One in-world purchase paid in Polygon MANA, as the shell keeps it (./purchase, behind `shop-mana-in-world`): the
// sale's terms the buyer confirmed, built with the rails' own encoders, and the attempt's record, which says how far
// the sale got on each way it was sent, so that a reload never sends it twice and never ends one that may have gone.
//
// The sale goes as the venue's meta-transaction through the relay, and, only when the relay proves it sent nothing
// (`{"ok":false,"not_sent":true}`), once from the buyer's wallet: the rails' own fallback (relayOrSend). Each way is a
// rail of the record, kept and read back from storage at two checkpoints:
//
//  - `signing`: before the wallet is asked to sign the meta-transaction, with the nonce the checked signature names.
//    The signature stays in this page: it leaves only through the relay request below.
//  - `dispatched`: right before the relay request carrying the sale, or the wallet's own send (the wallet signs and
//    sends at once, so its two checkpoints are one).
//
// Then `refused` when the relay answers that it sent nothing, and `declined` when the wallet answers that the buyer
// said no to its send (EIP-1193 4001). Nothing leaves this page without its checkpoint held: a request whose
// checkpoint couldn't be kept is answered here as never sent, and nothing more is sent for the purchase. So a record
// with no rail `dispatched` provably sent nothing, and one with a rail `dispatched` may have: it stays pending until a
// receipt for its own hash says how it ended. No timer, ownership read or nonce read ends it.
//
// The approve (MANA's allowance for the venue) is not the sale: it goes through the same guard, needs no checkpoint,
// and may be relayed or sent while the purchase is open. Once it is closed (the buyer cancelled), nothing more goes,
// an approve included.

import { encodeAbiParameters, keccak256, type Hex } from 'viem'
import type { Eip1193Provider } from '../../../data/auth/wallet'
import type { CartGroup } from '@shop/cart/partition'
import { refusedBeforeBroadcast } from '../../../shop/checkout/chain/metatx'
import { purchaseTargetFor, toUnits, type ResolvedLine } from '../../../shop/checkout/purchase/cart'
import { encodeMarketplaceCall } from '../../../shop/checkout/purchase/encoding'
import { manaSpenderFor } from '../../../shop/checkout/purchase/mana'
import { tokenRefId } from '@shop/model/asset'
import type { ManaHandOff, ManaSaleTerms } from './manaGuard'

export type ManaRailStage = 'signing' | 'dispatched' | 'refused' | 'declined'

/** One way the sale was sent. */
export interface ManaRail {
  rail: 'relay' | 'direct'
  /** The meta-transaction nonce the relayed signature names, in decimal. */
  nonce?: string
  stage: ManaRailStage
}

/** A MANA purchase's attempt, kept in its operation's record (./ledger). */
export interface ManaAttemptRecord {
  /** The venue the sale is for (the marketplace or the CollectionStore), lowercase. */
  venue: string
  /** The confirmed sale (ManaSaleTerms), hashed: its exact listing calldata, or its mints' units and ceiling. */
  fingerprint: string
  /** The rails tried, in order: at most the relay, then the buyer's wallet. */
  rails: ManaRail[]
}

const STAGES: readonly ManaRailStage[] = ['signing', 'dispatched', 'refused', 'declined']
const ADDRESS = /^0x[0-9a-f]{40}$/
const HASH = /^0x[0-9a-f]{64}$/

const isObject = (v: unknown): v is Record<string, unknown> => v != null && typeof v === 'object' && !Array.isArray(v)

function isRail(v: unknown): v is ManaRail {
  return (
    isObject(v) &&
    (v.rail === 'relay' || v.rail === 'direct') &&
    STAGES.includes(v.stage as ManaRailStage) &&
    (v.nonce === undefined || (v.rail === 'relay' && typeof v.nonce === 'string' && /^\d{1,78}$/.test(v.nonce))) &&
    (v.rail === 'relay' || v.stage !== 'refused')
  )
}

/** A stored attempt, checked whole: another version of this page, or anything on the origin, may have written it. */
export function isManaAttempt(v: unknown): v is ManaAttemptRecord {
  return (
    isObject(v) &&
    typeof v.venue === 'string' &&
    ADDRESS.test(v.venue) &&
    typeof v.fingerprint === 'string' &&
    HASH.test(v.fingerprint) &&
    Array.isArray(v.rails) &&
    v.rails.length <= 2 &&
    v.rails.every(isRail)
  )
}

/** Whether the sale may have left this page: a rail got as far as its dispatch, and no answer since says it didn't go. */
export function manaDispatched(attempt: ManaAttemptRecord): boolean {
  return attempt.rails.some((r) => r.stage === 'dispatched')
}

/**
 * The sale the guard binds, from the group's reviewed lines, built as the executor's runManaGroup builds what it sends:
 * listings as `encodeMarketplaceCall` of their trades and coupons for the buyer, mints as the units and what the
 * group's quotes add up to.
 */
export function manaSaleTerms(group: CartGroup, lines: ResolvedLine[], buyer: string): ManaSaleTerms {
  const units = toUnits(lines)
  if (units.length === 0) throw new Error('A MANA purchase buys at least one unit')
  const targets = units.map(purchaseTargetFor)
  const venue = manaSpenderFor('mana', targets[0]).spender.toLowerCase()
  if (targets.some((t) => manaSpenderFor('mana', t).spender.toLowerCase() !== venue)) throw new Error('A MANA purchase pays one venue')
  const confirmed = units.map((u) => ({ contract: u.line.asset.contract.toLowerCase(), itemId: tokenRefId(u.line.asset.token) }))
  if (group.key.call.kind === 'storeBuy') {
    if (targets.some((t) => t.kind !== 'store')) throw new Error('A store group holds only mints')
    const maxWei = units.reduce((sum, u) => sum + (u.quote.method === 'mana' ? u.quote.amount : 0n), 0n)
    return { kind: 'mints', venue, units: confirmed, maxWei }
  }
  const trades = targets.flatMap((t) => (t.kind === 'trade' ? [t.trade] : []))
  if (trades.length !== targets.length) throw new Error('A marketplace group holds only listings')
  const coupons = targets.flatMap((t) => (t.kind === 'trade' && t.coupon ? [t.coupon] : []))
  return { kind: 'listings', venue, calldata: encodeMarketplaceCall(trades, buyer, coupons.length ? coupons : undefined), units: confirmed }
}

/** The confirmed sale, hashed: two terms with the same fingerprint are the same sale. */
export function manaFingerprint(terms: ManaSaleTerms): Hex {
  const units = terms.units.map((u) => `${u.contract.toLowerCase()}:${u.itemId}`).join(',')
  return terms.kind === 'listings'
    ? keccak256(encodeAbiParameters([{ type: 'string' }, { type: 'address' }, { type: 'string' }, { type: 'bytes' }], ['listings', terms.venue as Hex, units, terms.calldata]))
    : keccak256(encodeAbiParameters([{ type: 'string' }, { type: 'address' }, { type: 'string' }, { type: 'uint256' }], ['mints', terms.venue as Hex, units, terms.maxWei]))
}

export interface ManaAttempt {
  readonly record: ManaAttemptRecord
  /** The guard's hand-off (ManaPurchaseIntent.handOff): keeps the rail's checkpoint, or refuses it. */
  handOff(attempt: ManaHandOff): boolean
  /** The rails' relay transport: the sale's request only with its checkpoint kept; nothing at all once closed. */
  relay(send: typeof fetch): typeof fetch
  /** The buyer's wallet, under the guard: a send of the sale the buyer declines there ends its rail as declined. */
  observe(wallet: Eip1193Provider): Eip1193Provider
  /** Whether the sale may have left this page. */
  dispatched(): boolean
  /** Whether anything more may go for the purchase. */
  open(): boolean
  /** Nothing more goes for the purchase (the buyer cancelled it): neither an approve nor the sale. */
  close(): void
}

/** What a request that can't go answers, as the relay answers a refusal that sent nothing. */
const notSent = (message: string): Response =>
  new Response(JSON.stringify({ ok: false, not_sent: true, message }), { status: 409, headers: { 'content-type': 'application/json' } })

/** The contract a relay request is for: `params[0]` of its transaction data, lowercase; null when it reads otherwise. */
function relayTarget(init: RequestInit | undefined): string | null {
  if (typeof init?.body !== 'string') return null
  try {
    const target = (JSON.parse(init.body) as { transactionData?: { params?: unknown[] } }).transactionData?.params?.[0]
    return typeof target === 'string' ? target.toLowerCase() : null
  } catch {
    return null
  }
}

const isDecline = (e: unknown): boolean => (e as { code?: unknown } | null)?.code === 4001

/**
 * A purchase's attempt at `terms`, with no rail yet. `keep` writes the record and reads it back from storage: true
 * only when what storage holds is the record given.
 */
export function createManaAttempt(terms: ManaSaleTerms, keep: (record: ManaAttemptRecord) => boolean): ManaAttempt {
  const record: ManaAttemptRecord = { venue: terms.venue.toLowerCase(), fingerprint: manaFingerprint(terms), rails: [] }
  /** Set once a checkpoint couldn't be kept: nothing more is sent for the purchase. */
  let blocked = false
  let closed = false
  /** Keeps `next` in place of the record's rails, or leaves them as they were and blocks the purchase. */
  const move = (next: ManaRail[]): boolean => {
    const before = record.rails
    record.rails = next
    if (keep(structuredClone(record))) return true
    record.rails = before
    blocked = true
    return false
  }
  const last = (): ManaRail | undefined => record.rails[record.rails.length - 1]
  const restage = (stage: ManaRailStage): boolean => {
    const rail = last()
    return rail ? move([...record.rails.slice(0, -1), { ...rail, stage }]) : false
  }
  return {
    record,
    handOff(attempt) {
      if (blocked || closed) return false
      if (attempt.rail === 'relay') {
        // The first way only, and one signature for it.
        if (record.rails.length > 0) return false
        return move([{ rail: 'relay', nonce: attempt.nonce.toString(), stage: 'signing' }])
      }
      // The wallet's send: the first way, or the one after a relay that sent nothing (it refused the sale, or never got
      // it). Never next to a send that may have gone, and never twice.
      if (record.rails.some((r) => r.rail === 'direct' || r.stage === 'dispatched')) return false
      return move([...record.rails, { rail: 'direct', stage: 'dispatched' }])
    },
    relay(send) {
      return (async (input: Parameters<typeof fetch>[0], init?: RequestInit) => {
        if ((init?.method ?? 'GET').toUpperCase() !== 'POST') return send(input, init)
        if (closed) return notSent('This purchase was cancelled, so nothing more was sent for it.')
        if (relayTarget(init) !== record.venue) return send(input, init)
        // The sale: its signature was kept (`signing`), and now its dispatch, before the request leaves.
        const rail = last()
        if (blocked || rail?.rail !== 'relay' || rail.stage !== 'signing' || !restage('dispatched')) {
          blocked = true
          return notSent("This browser couldn't keep a record of the purchase, so it wasn't sent.")
        }
        const res = await send(input, init)
        const body = await res
          .clone()
          .json()
          .catch(() => null)
        // The relay's own evidence that nothing went: the rails may send it once from the wallet. Unkept, they may not.
        if (refusedBeforeBroadcast(body) && !restage('refused')) blocked = true
        return res
      }) as typeof fetch
    },
    observe(wallet) {
      return {
        async request(args) {
          try {
            return await wallet.request(args)
          } catch (e) {
            const tx = (Array.isArray(args.params) ? args.params[0] : null) as { to?: unknown } | null
            const rail = last()
            const sale = args.method === 'eth_sendTransaction' && typeof tx?.to === 'string' && tx.to.toLowerCase() === record.venue
            // Declined in the wallet: the wallet's answer that it sent nothing. Kept or not, nothing more goes after it.
            if (sale && isDecline(e) && rail?.rail === 'direct' && rail.stage === 'dispatched') {
              if (!restage('declined')) blocked = true
            }
            throw e
          }
        },
        ...(wallet.on ? { on: wallet.on.bind(wallet) } : {}),
        ...(wallet.removeListener ? { removeListener: wallet.removeListener.bind(wallet) } : {})
      }
    },
    dispatched: () => manaDispatched(record),
    open: () => !closed,
    close() {
      closed = true
    }
  }
}
