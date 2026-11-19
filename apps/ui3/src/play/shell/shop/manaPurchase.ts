// One in-world purchase paid in Polygon MANA, as the shell keeps it (./purchase, behind `shop-mana-in-world`): the
// sale's terms the buyer confirmed, built with the rails' own encoders, and the attempt's record, which says how far
// each send of the sale got, so that a reload never sends it twice and never ends one that may have gone.
//
// The sale goes only through the relay, as the venue's meta-transaction (the rails' `relayOnly`): never from the
// buyer's wallet. Its signature names the buyer's meta-transaction nonce N at the venue, and every send of the sale (the
// first, and any retry) signs that same N with the same call, so on-chain at most one of them can ever run. The record
// keeps, from the first signature on:
//
//  - the sale's terms, its nonce N and its exact call (`data`), before the wallet is asked to sign;
//  - before the first relay request leaves, a block checkpoint at or before it (`fromBlock`, where the chain is read
//    from), and for each send, right before its request, the digest of the signed execution it carries (`payload`)
//    and when it went (`at`).
//
// A send is `signing` (kept before its signature), then `dispatched` (kept right before its relay request), and then
// `refused` when the relay answers that it sent nothing, or `declined` when the buyer said no to its signature in the
// wallet (EIP-1193 4001). Nothing leaves this page without its checkpoint held: a request whose checkpoint couldn't be
// kept is answered here as never sent, and nothing more is sent. So a record with no send `dispatched` provably sent
// nothing. One with a send `dispatched` may have: the chain says how it ended (./manaEvidence), never a timer.
//
// The approve (MANA's allowance for the venue) is not the sale: it needs no checkpoint, and goes only while the buyer's
// own AUTHORIZE runs it (`authorize`): the guard asks `approving()`, and the relay transport takes a request for MANA
// only then. Any other relay request (another contract, or one it can't read) is answered as never sent. Once the
// purchase is closed (the buyer cancelled), nothing more goes, an approve included.

import { encodeAbiParameters, keccak256, type Hex } from 'viem'
import type { Eip1193Provider } from '../../../data/auth/wallet'
import type { CartGroup } from '@shop/cart/partition'
import { refusedBeforeBroadcast } from '../../../shop/checkout/chain/metatx'
import { purchaseTargetFor, toUnits, type ResolvedLine } from '../../../shop/checkout/purchase/cart'
import { encodeMarketplaceCall } from '../../../shop/checkout/purchase/encoding'
import { manaSpenderFor } from '../../../shop/checkout/purchase/mana'
import { tokenRefId } from '@shop/model/asset'
import type { ManaHandOff, ManaSaleTerms } from './manaGuard'
import { deadline, DeadlineError } from './manaEvidence'

type SendStage = 'signing' | 'dispatched' | 'refused' | 'declined'

/** One send of the sale through the relay: the first, or a retry of it at the same nonce. */
interface ManaSend {
  /** This send's own name, kept with it from its signature on: two sends of the same sale can carry the same bytes. */
  id: string
  stage: SendStage
  /** keccak256 of the signed execution its relay request carried (executeMetaTransaction's call data). */
  payload?: string
  /** The hash the relay answered for it. */
  hash?: string
  /** When its request went (ms). */
  at?: number
}

/** The confirmed sale, as storage keeps it (ManaSaleTerms, its amounts in decimal). */
type StoredTerms =
  | { kind: 'listings'; venue: string; calldata: Hex; units: { contract: string; itemId: string }[] }
  | { kind: 'mints'; venue: string; units: { contract: string; itemId: string }[]; maxWei: string }

/** A MANA purchase's attempt, kept in its operation's record (./ledger). */
export interface ManaAttemptRecord {
  /** The venue the sale is for (the marketplace or the CollectionStore), lowercase. */
  venue: string
  /** The confirmed sale, as the guard binds it: a retry binds exactly this. */
  terms: StoredTerms
  /** The terms, hashed: two terms with the same fingerprint are the same sale. */
  fingerprint: string
  /** The meta-transaction nonce every send signs, in decimal. */
  nonce?: string
  /** The venue's call every send signs, exactly. */
  data?: Hex
  /** A block at or before the first relay request: the chain is read from here. */
  fromBlock?: string
  /** Each send, in order: the first, then any retries. */
  sends: ManaSend[]
}

/**
 * The sends a record keeps: the first and its retries, each one's evidence (the payload that went). At that many sends
 * that went, no more retries: the evidence is all kept, and the chain still decides.
 */
export const MAX_SENDS = 8
const STAGES: readonly SendStage[] = ['signing', 'dispatched', 'refused', 'declined']
/** How long a relay request of the sale may go unanswered before it is taken as a relay that didn't answer. */
export const SEND_MS = 30_000
const ADDRESS = /^0x[0-9a-f]{40}$/
const HASH = /^0x[0-9a-f]{64}$/
const TX_HASH = /^0x[0-9a-fA-F]{64}$/
const DECIMAL = /^\d{1,78}$/
const HEX = /^0x[0-9a-fA-F]*$/

const isObject = (v: unknown): v is Record<string, unknown> => v != null && typeof v === 'object' && !Array.isArray(v)
const isUnits = (v: unknown): boolean =>
  Array.isArray(v) && v.length > 0 && v.every((u) => isObject(u) && typeof u.contract === 'string' && ADDRESS.test(u.contract) && typeof u.itemId === 'string' && DECIMAL.test(u.itemId))

function isSend(v: unknown): v is ManaSend {
  return (
    isObject(v) &&
    typeof v.id === 'string' &&
    /^[0-9a-z-]{1,64}$/.test(v.id) &&
    STAGES.includes(v.stage as SendStage) &&
    (v.payload === undefined || (typeof v.payload === 'string' && HASH.test(v.payload))) &&
    (v.hash === undefined || (typeof v.hash === 'string' && TX_HASH.test(v.hash))) &&
    (v.at === undefined || (typeof v.at === 'number' && Number.isFinite(v.at) && v.at >= 0)) &&
    // A send that went carries what went.
    (v.stage !== 'dispatched' || (typeof v.payload === 'string' && typeof v.at === 'number'))
  )
}

function isTerms(v: unknown, venue: string): v is StoredTerms {
  if (!isObject(v) || v.venue !== venue || !isUnits(v.units)) return false
  if (v.kind === 'listings') return typeof v.calldata === 'string' && HEX.test(v.calldata)
  return v.kind === 'mints' && typeof v.maxWei === 'string' && DECIMAL.test(v.maxWei)
}

/** A stored attempt, checked whole: another version of this page, or anything on the origin, may have written it. */
export function isManaAttempt(v: unknown): v is ManaAttemptRecord {
  if (!isObject(v) || typeof v.venue !== 'string' || !ADDRESS.test(v.venue)) return false
  const sends = v.sends
  return (
    isTerms(v.terms, v.venue) &&
    typeof v.fingerprint === 'string' &&
    HASH.test(v.fingerprint) &&
    (v.nonce === undefined || (typeof v.nonce === 'string' && DECIMAL.test(v.nonce))) &&
    (v.data === undefined || (typeof v.data === 'string' && HEX.test(v.data))) &&
    (v.fromBlock === undefined || (typeof v.fromBlock === 'string' && DECIMAL.test(v.fromBlock))) &&
    Array.isArray(sends) &&
    sends.length <= MAX_SENDS &&
    sends.every(isSend) &&
    new Set(sends.map((x) => (x as ManaSend).id)).size === sends.length &&
    // A signature names a nonce and a call, and a dispatch has its checkpoint.
    (sends.length === 0 || (v.nonce !== undefined && v.data !== undefined)) &&
    (!sends.some((s) => (s as ManaSend).stage === 'dispatched') || v.fromBlock !== undefined)
  )
}

/** Whether the sale may have left this page: a send got as far as its relay request, and no answer since says it didn't. */
export function manaDispatched(attempt: ManaAttemptRecord): boolean {
  return attempt.sends.some((s) => s.stage === 'dispatched')
}

/**
 * The record with only what may be evidence: the sends that went (`dispatched`), each one kept as itself (two may carry
 * the same bytes, and the relay may still answer either), in order. A send still `signing` with no sender left to finish
 * it, one the relay refused or one the buyer declined never left this page, so nothing on chain can be it.
 */
export function compactManaAttempt(record: ManaAttemptRecord): ManaAttemptRecord {
  return { ...record, sends: record.sends.filter((s) => s.stage === 'dispatched').map((s) => ({ ...s })) }
}

/** Whether the record has room for another send: none being signed, and fewer than MAX_SENDS that went. */
export function manaHasRoom(record: ManaAttemptRecord): boolean {
  return !record.sends.some((s) => s.stage === 'signing') && record.sends.filter((s) => s.stage === 'dispatched').length < MAX_SENDS
}

/** When the first send that went, went (ms), or null. */
export function manaFirstDispatch(attempt: ManaAttemptRecord): number | null {
  const at = attempt.sends.flatMap((s) => (s.stage === 'dispatched' && s.at != null ? [s.at] : []))
  return at.length > 0 ? Math.min(...at) : null
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
  // One payment entry per listing, as the server prices it (from its first): a second would be paid beyond that price.
  if (trades.some((t) => t.received.length !== 1)) throw new Error('A listing is paid in one asset')
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

const storeTerms = (terms: ManaSaleTerms): StoredTerms =>
  terms.kind === 'listings'
    ? { kind: 'listings', venue: terms.venue.toLowerCase(), calldata: terms.calldata, units: terms.units }
    : { kind: 'mints', venue: terms.venue.toLowerCase(), units: terms.units, maxWei: terms.maxWei.toString() }

/** The confirmed sale a record keeps, as the guard binds it. */
export function storedManaTerms(record: ManaAttemptRecord): ManaSaleTerms {
  const t = record.terms
  return t.kind === 'listings' ? { kind: 'listings', venue: t.venue, calldata: t.calldata, units: t.units } : { kind: 'mints', venue: t.venue, units: t.units, maxWei: BigInt(t.maxWei) }
}

export interface ManaAttempt {
  readonly record: ManaAttemptRecord
  /** The guard's hand-off (ManaPurchaseIntent.handOff): keeps the send's checkpoint, or refuses it. */
  handOff(attempt: ManaHandOff): boolean
  /** The relay transport: the sale's request only with its checkpoints kept, MANA's approve only while authorizing. */
  relay(send: typeof fetch): typeof fetch
  /** Runs the buyer's AUTHORIZE: the only time MANA's approve may go. */
  authorize<T>(run: () => Promise<T>): Promise<T>
  /** Whether an approve may go now (ManaPurchaseIntent.mayApprove). */
  approving(): boolean
  /** Lets one more send of the same sale, at the same nonce, be handed off: the buyer's RETRY. */
  retry(): void
  /** Keeps the hash the relay answered for the latest send. */
  answered(hash: string): void
  /** Drops what can't be evidence (compactManaAttempt): sends that never left, once no sender is left to finish them. */
  retire(): boolean
  /** The buyer's wallet, under the guard: a signature of the sale the buyer declines there ends its send as declined. */
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

/** The method of a request to the relay, whichever shape it is given in. */
function methodOf(input: Parameters<typeof fetch>[0], init: RequestInit | undefined): string {
  return (init?.method ?? (typeof Request !== 'undefined' && input instanceof Request ? input.method : 'GET')).toUpperCase()
}

/** A relay request's contract (`params[0]`) and payload (`params[1]`), as read from its body; null when it reads otherwise. */
function relayRequest(input: Parameters<typeof fetch>[0], init: RequestInit | undefined): { target: string; payload: Hex } | null {
  if (typeof Request !== 'undefined' && input instanceof Request) return null
  if (typeof init?.body !== 'string') return null
  try {
    const params = (JSON.parse(init.body) as { transactionData?: { params?: unknown[] } }).transactionData?.params
    const [target, payload] = Array.isArray(params) ? params : []
    return typeof target === 'string' && typeof payload === 'string' && HEX.test(payload) ? { target: target.toLowerCase(), payload: payload as Hex } : null
  } catch {
    return null
  }
}

const isDecline = (e: unknown): boolean => (e as { code?: unknown } | null)?.code === 4001

/** A new send's name. */
const sendId = (): string => (typeof crypto !== 'undefined' && 'randomUUID' in crypto ? crypto.randomUUID() : `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`)

/** A relay answer read whole: its status, headers and body text. */
type Answer = { status: number; headers: Headers; text: string }

/**
 * A purchase's attempt: a fresh one at `terms` (no send yet), or `from` a kept record (a retry after a reload).
 *
 * The record lives with the operation, not in the attempt: every change reads it from `load` (the operation's current
 * record, which another attempt of the same operation, a retry, may have changed meanwhile) and writes it through
 * `keep`, which reads it back from storage (true only when storage holds the record given). An answer the relay gives
 * late lands on its own send, by that send's name, in whatever the record has become.
 *
 * `mana`: the chain's MANA token, whose approve the relay may carry while the buyer authorizes it. `checkpoint` reads a
 * block at or before now, kept before the first relay request; `now` stamps each send. `onDispatch` hears each send kept
 * as gone, right before its request leaves: the purchase is followed from then on, not once its answer comes. `sendMs`
 * bounds a relay request, its whole answer included.
 */
export function createManaAttempt(
  terms: ManaSaleTerms,
  keep: (record: ManaAttemptRecord) => boolean,
  opts: {
    mana: string
    checkpoint: () => Promise<bigint>
    now: () => number
    from?: ManaAttemptRecord
    load?: () => ManaAttemptRecord | undefined
    onDispatch?: () => void
    sendMs?: number
  }
): ManaAttempt {
  let local: ManaAttemptRecord = opts.from
    ? structuredClone(opts.from)
    : { venue: terms.venue.toLowerCase(), terms: storeTerms(terms), fingerprint: manaFingerprint(terms), sends: [] }
  if (local.fingerprint !== manaFingerprint(terms)) throw new Error('The kept attempt is for another sale')
  /** The operation's record as it stands now. */
  const read = (): ManaAttemptRecord => {
    const shared = opts.load?.()
    return shared && shared.fingerprint === local.fingerprint ? shared : local
  }
  const mana = opts.mana.toLowerCase()
  /** Set once a checkpoint couldn't be kept: nothing more is sent for the purchase. */
  let blocked = false
  let closed = false
  /** The buyer's AUTHORIZE is running. */
  let authorizing = 0
  /** The buyer's RETRY lets one more send be handed off. */
  let retrying = false
  /** The send this attempt is signing or sending now. */
  let current: string | null = null
  /** Keeps `next` as the record, or leaves it as it was and blocks the purchase. */
  const move = (next: ManaAttemptRecord): boolean => {
    if (!keep(structuredClone(next))) {
      blocked = true
      return false
    }
    local = structuredClone(next)
    return true
  }
  /** Changes the send named `id` in the record as it stands now; false when it is gone or couldn't be kept. */
  const change = (id: string | null, patch: Partial<ManaSend>): boolean => {
    const now = read()
    const at = id == null ? -1 : now.sends.findIndex((x) => x.id === id)
    return at >= 0 && move({ ...now, sends: now.sends.map((x, i) => (i === at ? { ...x, ...patch } : x)) })
  }
  const send = (id: string | null): ManaSend | undefined => (id == null ? undefined : read().sends.find((x) => x.id === id))
  /** The relay's word in `answer` for the send named `id`: that nothing went (`refused`), or the hash it gave. */
  const answerFor = (id: string, answer: Answer): void => {
    let body: { ok?: unknown; txHash?: unknown } | null = null
    try {
      body = JSON.parse(answer.text) as { ok?: unknown; txHash?: unknown }
    } catch {
      body = null
    }
    if (send(id)?.stage !== 'dispatched') return
    if (refusedBeforeBroadcast(body)) change(id, { stage: 'refused' })
    else if (body?.ok !== false && typeof body?.txHash === 'string' && TX_HASH.test(body.txHash)) change(id, { hash: body.txHash })
  }
  /** A wallet request for the sale's signature: the typed data's domain is the venue's. */
  const saleSignature = (args: { method: string; params?: unknown }): boolean => {
    const params = Array.isArray(args.params) ? args.params : []
    if (args.method !== 'eth_signTypedData_v4' || typeof params[1] !== 'string') return false
    try {
      const domain = (JSON.parse(params[1]) as { domain?: { verifyingContract?: unknown } }).domain
      return typeof domain?.verifyingContract === 'string' && domain.verifyingContract.toLowerCase() === read().venue
    } catch {
      return false
    }
  }
  return {
    get record() {
      return read()
    },
    handOff(attempt) {
      // Only through the relay, and never past a checkpoint that couldn't be kept or a purchase closed.
      if (blocked || closed || attempt.rail !== 'relay') return false
      const now = read()
      const id = sendId()
      if (now.sends.length === 0) {
        if (!move({ ...now, nonce: attempt.nonce.toString(), data: attempt.data, sends: [{ id, stage: 'signing' }] })) return false
        current = id
        return true
      }
      // A retry: the buyer's own, of the same call at the same nonce, while no send is being signed and the record has room.
      if (!retrying || !manaHasRoom(compactManaAttempt(now)) || now.sends.some((x) => x.stage === 'signing')) return false
      if (attempt.nonce.toString() !== now.nonce || attempt.data.toLowerCase() !== now.data?.toLowerCase()) return false
      retrying = false
      const kept = compactManaAttempt(now)
      if (!move({ ...kept, sends: [...kept.sends, { id, stage: 'signing' }] })) return false
      current = id
      return true
    },
    relay(post) {
      return (async (input: Parameters<typeof fetch>[0], init?: RequestInit) => {
        if (methodOf(input, init) !== 'POST') return post(input, init)
        if (closed) return notSent('This purchase was cancelled, so nothing more was sent for it.')
        const request = relayRequest(input, init)
        if (request?.target === mana) return authorizing > 0 ? post(input, init) : notSent('MANA is approved only when you authorize it, so nothing was sent.')
        if (request?.target !== read().venue) return notSent('The Shop sends only this purchase, so nothing was sent.')
        // The sale: its signature was kept (`signing`); now its checkpoints, before the request leaves.
        const id = current
        if (blocked || id == null || send(id)?.stage !== 'signing') {
          blocked = true
          return notSent("This browser couldn't keep a record of the purchase, so it wasn't sent.")
        }
        let fromBlock = read().fromBlock
        if (fromBlock == null) {
          try {
            fromBlock = (await opts.checkpoint()).toString()
          } catch {
            blocked = true
            return notSent("This purchase couldn't be checked on chain, so it wasn't sent.")
          }
        }
        const now = read()
        const at = now.sends.findIndex((x) => x.id === id)
        if (at < 0 || !move({ ...now, fromBlock, sends: now.sends.map((x, i) => (i === at ? { ...x, stage: 'dispatched', payload: keccak256(request.payload), at: opts.now() } : x)) })) {
          return notSent("This browser couldn't keep a record of the purchase, so it wasn't sent.")
        }
        current = null
        // Kept as gone: whoever follows the purchase takes over from here, whatever the request and its answer take.
        opts.onDispatch?.()
        // The whole answer, body included, read under the deadline; the rails get it buffered.
        const answering = (async (): Promise<Answer> => {
          const res = await post(input, init)
          return { status: res.status, headers: new Headers(res.headers), text: await res.text() }
        })()
        // An answer past the deadline still lands on its own send.
        let late = false
        answering.then(
          (answer) => {
            if (late) answerFor(id, answer)
          },
          () => {}
        )
        let answer: Answer
        try {
          answer = await deadline(answering, opts.sendMs ?? SEND_MS, 'The relay')
        } catch (e) {
          // No whole answer in time: the request may have reached the relay, so the send stays dispatched, and the rails
          // take it as a relay that didn't answer (followed, never sent again from here).
          if (e instanceof DeadlineError) {
            late = true
            throw new TypeError(e.message)
          }
          throw e
        }
        answerFor(id, answer)
        return new Response(answer.text, { status: answer.status, headers: answer.headers })
      }) as typeof fetch
    },
    async authorize(run) {
      authorizing++
      try {
        return await run()
      } finally {
        authorizing--
      }
    },
    approving: () => authorizing > 0 && !closed,
    retry() {
      retrying = true
    },
    answered(hash) {
      const latest = read().sends.filter((x) => x.stage === 'dispatched').at(-1)
      if (latest && !latest.hash && TX_HASH.test(hash)) change(latest.id, { hash })
    },
    retire() {
      const now = read()
      const kept = compactManaAttempt(now)
      return JSON.stringify(kept) === JSON.stringify(now) || move(kept)
    },
    observe(wallet) {
      return {
        async request(args) {
          try {
            return await wallet.request(args)
          } catch (e) {
            // Declined in the wallet: the buyer said no to this send's signature. Nothing of it went.
            if (isDecline(e) && saleSignature(args) && send(current)?.stage === 'signing') change(current, { stage: 'declined' })
            throw e
          }
        },
        ...(wallet.on ? { on: wallet.on.bind(wallet) } : {}),
        ...(wallet.removeListener ? { removeListener: wallet.removeListener.bind(wallet) } : {})
      }
    },
    dispatched: () => manaDispatched(read()),
    open: () => !closed,
    close() {
      closed = true
    }
  }
}
