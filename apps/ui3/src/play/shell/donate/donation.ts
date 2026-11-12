// The web shell's side of Donate (lib/donateProtocol). The HUD names a place; this resolves the place's creator
// itself (the places service), reads the creator's profile, the player's MANA balance, the MANA price and the
// suggested amounts, and shows the player the panel over the app (./overlay). On SEND it transfers exactly the
// confirmed MANA to that creator with the wallet the host injected, bound to that tip (./guard): a gasless
// meta-transaction through the relay, and nothing else (a transaction from the wallet beside it could pay twice).
// The relay's refusal fails the tip; trying again signs the same nonce, so at most one can land. An outcome it can't
// tell (the relay's answer lost, or the transaction not yet mined) is shown as processing and watched for the tip's
// own Transfer, never retried. Once the tip is mined, the shell
// forwards its hash to the catalyst, signed by the login, which then tells the creator (`tip_received`), and posts
// `bevy-shell:donate` so the app plays the tip emote.
//
// Donate is offered to a web3 wallet sign-in only (AGENTS.md: MANA is shown to logged-in web3 wallets), and that is
// checked again here on every open, not only in the capabilities the HUD reads.

import type { Eip1193Provider } from '../../../data/auth/wallet'
import { catalystBase, getJSON } from '../../../data/catalyst/client'
import { fetchProfile, mapProfile, profileFaceUrl } from '../../../data/catalyst/profile'
import { encodeManaTransfer, fetchTipTarget, manaToWei, TIP_FORWARD_PATH, type TipTarget } from '../../../explorer/components/manaTip'
import { settlementChainId } from '../../../shop/checkout/chain/config'
import { flagsService } from '../../../shop/discovery/lib/remoteFlags'
import { parseRemoteFlags } from '@shop/discovery/lib/rolloutFlags'
import { manaToken, type ContractData } from '@shop/market/lib/contracts'
import { readManaBalance } from '../../../shop/market/lib/mana'
import { MetaTransactionError, readNonce, sendMetaTransaction } from '../../../shop/market/lib/metaTx'
import { rpc } from '../../../shop/market/lib/rpc'
import { isTxReverted, waitForReceipt } from '../../../shop/market/lib/tx'
import type { ChainId } from '@shop/model/chain'
import { loginExpired, rootAddress, type StoredLogin } from '@explorer-core/features/auth/sso'
import { siteUrl } from '../../lib/baseDomain'
import {
  DONATE_EVENT,
  DONATE_METHODS,
  DONATE_PROTOCOL,
  DONATE_UNAVAILABLE,
  MIN_TIP,
  TIP_AMOUNT,
  donateBusy,
  parseDonateRequest,
  parseDonateTarget,
  type DonateCapabilities,
  type DonateEvent,
  type DonatePlace,
  type DonateRequest
} from '../../lib/donateProtocol'
import { ShellError } from '@explorer-core/lib/shellReply'
import type { ShellShopWallet } from '../shop/purchase'
import { loginSigner } from '../loginSigner'
import { pageLocks, webLock } from '../webLock'
import { guardedDonationProvider, type DonationIntent } from './guard'
import type { DonateCreator, DonateHandlers, DonateView } from '@hud/features/donate/types'

/** The suggested amounts when the flags file names none. */
export const DEFAULT_PRESETS = [166, 333, 500]
const DONATIONS_FLAG = 'explorer-alfa-donations'
const PRESETS_FLAG = 'explorer-alfa-recommended-donation-amount'
const RATE_CACHE_MS = 30 * 60_000
/** How long the panel waits for the tip to be mined before it says the tip is processing. */
const RECEIPT_WAIT_MS = 3 * 60_000
/** How long a processing tip is still watched (by its effect on the balance) after the panel says so. */
const WATCH_FOR_MS = 30 * 60_000
const WATCH_EVERY_MS = 10_000
const FORWARD_RETRY_MS = [5_000, 10_000, 20_000, 40_000, 60_000]
const TARGET_TTL_MS = 5 * 60_000
const FORWARD_TIMEOUT_MS = 10_000

export type { DonateCreator, DonateHandlers, DonateView }

/** Draws the panel over the app, or takes it away. */
export interface ShellDonateUi {
  show(view: DonateView, handlers: DonateHandlers, opening?: { pointer: boolean }): void
  hide(): void
}

/** A place's creator and name, as the places service has them. */
export interface ResolvedPlace {
  creator: string
  title: string
  /** The place's parcels, when the service names them: each answers donate.target the same. */
  positions?: string[]
}

/** What Donate reads and runs, injectable for tests. */
export interface ShellDonateServices {
  enabled(): Promise<boolean>
  presets(): Promise<number[]>
  place(place: DonatePlace): Promise<ResolvedPlace | null>
  creator(address: string): Promise<DonateCreator>
  /** The account's MANA on the tip chain, in wei. */
  balance(account: string, token: ContractData): Promise<bigint>
  usdRate(): Promise<number | null>
  /** The chain and MANA token the deployment's payments config names, or null when it can't be read. */
  tipTarget(): Promise<TipTarget | null>
  chainId(): ChainId
  guard(provider: Eip1193Provider, intent: DonationIntent): Eip1193Provider
  /** Relays the token meta-transaction; `fetchImpl` carries the relay POST (its timeout, and whether it was made). */
  sendGasless(provider: Eip1193Provider, token: ContractData, data: `0x${string}`, fetchImpl: typeof fetch): Promise<string>
  /** A fresh id for a pending tip's record. */
  newId(): string
  /**
   * The account's tip lock (a Web Lock), so one tab at a time sends or watches its tip: taken only if free (null when
   * another tab holds it), or with `wait` once that tab lets go. Null too without Web Locks (`locksAvailable`).
   */
  claim(account: string, wait?: boolean): Promise<(() => void) | null>
  locksAvailable(): boolean
  /** Where an unsettled tip is kept across reloads and tabs; null when this browser keeps nothing. */
  storage(): Storage | null
  /** The chain's latest block, read before signing: where a tip's Transfer is looked for from. */
  blockNumber(chainId: ChainId): Promise<bigint>
  /** The token's Transfer events from `from` to `to` since `fromBlock`: their transaction and value. */
  transfers(token: ContractData, from: string, to: string, fromBlock: bigint): Promise<Array<{ hash: string; value: bigint }>>
  /** The token's meta-transaction nonce for the account: past a tip's signed nonce once that tip was used. */
  nonce(token: ContractData, account: string): Promise<bigint>
  /** Resolves when mined; TxRevertedError on a revert, TxPendingError after `timeoutMs`; stops (rejects) once `stop()`. */
  wait(chainId: ChainId, hash: string, timeoutMs: number, stop?: () => boolean): Promise<string>
  forward(hash: string, login: StoredLogin): Promise<void>
  sleep(ms: number): Promise<void>
  now(): number
  accountUrl(): string
  supportUrl(): string
}

export interface ShellDonateDeps {
  login(): StoredLogin | null
  wallet: ShellShopWallet | null
  ui: ShellDonateUi
  /** To the app frame. */
  post(event: DonateEvent): void
  openExternal(url: string): void
  /** Whether another dialog (the Shop's) is up over the app: Donate waits its turn. */
  otherDialogOpen(): boolean
  services?: Partial<ShellDonateServices>
}

export interface ShellDonate {
  /** Answers a `donate.*` request from the app frame. */
  request(method: string, params: unknown): Promise<unknown>
  /** Whether the Donate panel is up. */
  dialogOpen(): boolean
}

const lower = (s: string): string => s.toLowerCase()

/** The place's name in the panel: the place's title, a World's name without `.dcl.eth`. */
export function placeTitle(place: DonatePlace, title: string | null | undefined): string {
  if ('world' in place) return place.world.replace(/\.dcl\.eth$/, '')
  return title?.trim() || 'Unknown place'
}

/** The suggested amounts in a flags payload ("166,333,500"): the first three positive numbers, else the defaults. */
export function parsePresets(csv: string | null | undefined): number[] {
  const values = (csv ?? '')
    .split(',')
    .map((s) => Number(s.trim()))
    .filter((n) => Number.isFinite(n) && n > 0)
    .slice(0, 3)
  // A suggestion below the least tip could never be sent.
  const sendable = values.filter((n) => n >= MIN_TIP)
  return sendable.length >= 1 ? sendable : DEFAULT_PRESETS
}

/** MANA in wei as a number of MANA for display. */
export function weiToMana(wei: bigint): number {
  return Number(wei / 10n ** 14n) / 10_000
}

const FLAGS_TTL_MS = 60_000
const FLAGS_TIMEOUT_MS = 3_000
let flagsCache: { at: number; read: Promise<ReturnType<typeof parseRemoteFlags>> } | null = null

/** The Explorer's flags file (the same service the Shop reads its own from), kept a minute; a failed read is not kept. */
function readExplorerFlags(): Promise<ReturnType<typeof parseRemoteFlags>> {
  if (flagsCache && Date.now() - flagsCache.at < FLAGS_TTL_MS) return flagsCache.read
  const read = fetch(`${flagsService()}/explorer.json`, { signal: AbortSignal.timeout(FLAGS_TIMEOUT_MS) }).then(async (res) => {
    if (!res.ok) {
      await res.body?.cancel().catch(() => {})
      throw new Error(`feature flags: HTTP ${res.status}`)
    }
    return parseRemoteFlags(await res.json())
  })
  flagsCache = { at: Date.now(), read }
  read.catch(() => {
    if (flagsCache?.read === read) flagsCache = null
  })
  return read
}

let rateCache: { at: number; rate: number } | null = null

const LIVE: ShellDonateServices = {
  async enabled() {
    return (await readExplorerFlags()).flags[DONATIONS_FLAG] === true
  },
  async presets() {
    return parsePresets((await readExplorerFlags()).variants[PRESETS_FLAG])
  },
  async place(place) {
    // `world_names` matches a World's name exactly (`names` is a partial match); `positions` alone lists places only.
    const query = 'world' in place ? { world_names: place.world } : { positions: `${place.parcel.x},${place.parcel.y}` }
    const res = await getJSON<{ data?: DestinationRow[] }>('/api/destinations', { service: 'places', query })
    return resolvePlace(place, res?.data ?? [])
  },
  async creator(address) {
    const avatar = await fetchProfile(address).catch(() => null)
    if (!avatar) return { address }
    const p = mapProfile(avatar, address)
    return { address, name: p.name, claimed: p.hasClaimedName, nameColor: p.nameColor, face: profileFaceUrl(avatar) ?? undefined }
  },
  balance: async (account, token) => BigInt(await readManaBalance(token.chainId, account)),
  nonce: (token, account) => readNonce(token, account),
  async usdRate() {
    if (rateCache && Date.now() - rateCache.at < RATE_CACHE_MS) return rateCache.rate
    const res = await fetch(`${catalystBase()}/price/api/v3/simple/price?ids=decentraland&vs_currencies=usd`, { signal: AbortSignal.timeout(FLAGS_TIMEOUT_MS) })
    const body = (await res.json().catch(() => null)) as { decentraland?: { usd?: unknown } } | null
    const rate = body?.decentraland?.usd
    if (!res.ok || typeof rate !== 'number' || !(rate > 0)) return null
    rateCache = { at: Date.now(), rate }
    return rate
  },
  tipTarget: () => fetchTipTarget().catch(() => null),
  chainId: () => settlementChainId(),
  guard: guardedDonationProvider,
  sendGasless: (provider, token, data, fetchImpl) => sendMetaTransaction({ provider, contract: token, kind: 'collection', functionData: data, fetchImpl }),
  newId: () => crypto.randomUUID(),
  claim: (account, wait) => webLock(pageLocks(), `dcl-donate:account:${account}`, wait),
  locksAvailable: () => pageLocks() != null,
  storage: () => {
    try {
      return window.localStorage
    } catch {
      return null
    }
  },
  blockNumber: async (chainId) => BigInt(await rpc<string>(chainId, 'eth_blockNumber', [])),
  async transfers(token, from, to, fromBlock) {
    const topic = (a: string): string => `0x${a.slice(2).toLowerCase().padStart(64, '0')}`
    const logs = await rpc<Array<{ transactionHash?: string; data?: string }>>(token.chainId, 'eth_getLogs', [
      { address: token.address, fromBlock: `0x${fromBlock.toString(16)}`, toBlock: 'latest', topics: [TRANSFER_TOPIC, topic(from), topic(to)] }
    ])
    return (logs ?? []).flatMap((l) => (l.transactionHash && l.data ? [{ hash: l.transactionHash, value: BigInt(l.data) }] : []))
  },
  wait: (chainId, hash, timeoutMs, stop) =>
    waitForReceipt(chainId, hash, 'The tip', {
      timeoutMs,
      // Polls stop with the tip: once it settled, there is nothing left to wait for.
      sleep: (ms) => (stop?.() ? Promise.reject(new Error('settled')) : new Promise((resolve) => setTimeout(resolve, ms)))
    }),
  async forward(hash, login) {
    const signer = loginSigner(login)
    if (!signer) throw new Error('The sign-in has expired.')
    const url = `${catalystBase()}${TIP_FORWARD_PATH}`
    // Signed for the exact path requested, gateway prefix included: the catalyst checks the path it was called on.
    const headers = await signer.sign('POST', new URL(url).pathname, {})
    const res = await fetch(url, {
      method: 'POST',
      headers: { 'content-type': 'application/json', ...headers },
      body: JSON.stringify({ type: 'tip-received', transactionHash: hash }),
      signal: AbortSignal.timeout(FORWARD_TIMEOUT_MS)
    })
    await res.body?.cancel().catch(() => {})
    // A 4xx is the catalyst refusing the claim (a 429 is its rate limit, asked again later).
    if (res.status >= 400 && res.status < 500 && res.status !== 429) throw new ForwardRefused(res.status)
    if (!res.ok) throw new Error(`The tip forward answered ${res.status}`)
  },
  sleep: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
  now: () => Date.now(),
  accountUrl: () => siteUrl('/account'),
  supportUrl: () => siteUrl('/help')
}


/** A row of the places service's destinations, as far as Donate reads it. */
interface DestinationRow {
  creator_address?: string | null
  title?: string | null
  world?: boolean
  world_name?: string | null
  positions?: string[]
}

/** The place's creator and name from the service's rows: only a row that IS the place asked for, never another. */
export function resolvePlace(place: DonatePlace, rows: DestinationRow[]): ResolvedPlace | null {
  const row =
    'world' in place
      ? rows.find((r) => r.world === true && r.world_name?.toLowerCase() === place.world)
      : rows.find((r) => r.world !== true && (r.positions ?? []).includes(`${place.parcel.x},${place.parcel.y}`))
  const creator = row?.creator_address?.trim().toLowerCase()
  if (!row || !creator || !/^0x[0-9a-f]{40}$/.test(creator)) return null
  return { creator, title: placeTitle(place, row.title), positions: row.positions }
}

/** The catalyst refused a tip's forward (a 4xx other than its rate limit): asking again changes nothing. */
class ForwardRefused extends Error {
  constructor(readonly status: number) {
    super(`The tip forward was refused (${status})`)
  }
}

/**
 * The relay answered and refused the tip (`ok: false`). It may have broadcast and lost it first, but a refused tip
 * isn't mined, and trying again signs the same meta-transaction nonce, so at most one of the two can land.
 */
function refusedByRelay(e: unknown): boolean {
  return e instanceof MetaTransactionError && e.refused === true
}

/** keccak256("Transfer(address,address,uint256)"). */
const TRANSFER_TOPIC = '0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef'

/** How an unsettled tip ended: mined, reverted, or still unknown when the watch ran out. */
type Outcome = 'sent' | 'failed' | 'unknown'

/** A signed tip as kept across reloads and tabs, per account, until it settles or its watch ends. */
interface PendingRecord {
  /** Which signature this is: a tab removes the stored record only while it still holds this one. */
  id: string
  amount: string
  creator: DonateCreator
  /** The tip, in wei. */
  wei: string
  /** The chain's block just before signing, as a decimal string: the tip's Transfer is looked for from it. */
  fromBlock: string
  /** The meta-transaction nonce the tip signed, as a decimal string: once the token's nonce is past it, it was used. */
  nonce: string
  /** Its transaction hash, once the relay or the wallet gave one. */
  hash?: string
  /** When the watch ends, in ms. */
  until: number
}

/** A tip signed but not yet settled. The panel showing it (if any) sets `onSettle`. */
interface Pending {
  record: PendingRecord
  onSettle: ((outcome: Outcome) => void) | null
  /** Shows the tip as processing in the panel that sent it, when a check sends it on to the watch. */
  onShowProcessing?: () => void
  /** Lets the account's tip lock go, once the tip settles. */
  release?: () => void
  settled: boolean
}

/** The nonce an `eth_signTypedData_v4` request signs (its typed data's `message.nonce`), or null. */
function signedNonce(params: unknown): string | null {
  const args = Array.isArray(params) ? params : []
  try {
    const typed = (typeof args[1] === 'string' ? JSON.parse(args[1]) : args[1]) as { message?: { nonce?: unknown } } | null
    const nonce = typed?.message?.nonce
    const text = typeof nonce === 'bigint' || typeof nonce === 'number' ? nonce.toString() : nonce
    return typeof text === 'string' && /^\d{1,78}$/.test(text) ? text : null
  } catch {
    return null
  }
}

const RELAY_TIMEOUT_MS = 60_000
/** How long each read the panel waits on may take before the panel closes. */
const LOAD_TIMEOUT_MS = 20_000
const storeKey = (account: string): string => `dcl:donate:pending:v2:${account}`

export function createShellDonate(deps: ShellDonateDeps): ShellDonate {
  const svc: ShellDonateServices = { ...LIVE, ...deps.services }
  /**
   * The open panel, or null. Each opening is its own session: a late answer for an older one draws nothing. It is
   * claimed before the first read, so the Shop (and a second Donate) see the dialog as taken from the start.
   */
  let session: { id: number } | null = null
  let sessions = 0
  /** Places looked up for donate.target, kept a while so walking around reads each once. Failed reads aren't kept. */
  const targets = new Map<string, { at: number; tippable: boolean }>()
  /** Unsettled tips, by account; also kept in storage, so a reload or another tab sees them. */
  const pending = new Map<string, Pending>()

  const account = (): string | null => {
    const login = deps.login()
    return login && !loginExpired(login) ? lower(rootAddress(login.identity)) : null
  }
  const web3 = (who: string): boolean => deps.wallet?.marketplace?.(who) === true && deps.wallet.kind(who) === 'web3'
  const tipped = (): void => deps.post({ type: DONATE_EVENT, v: DONATE_PROTOCOL, state: 'sent' })

  function save(who: string, record: PendingRecord | null): void {
    try {
      const store = svc.storage()
      if (!store) return
      if (record) store.setItem(storeKey(who), JSON.stringify(record))
      else store.removeItem(storeKey(who))
    } catch {
      // Kept in memory only: a reload loses it.
    }
  }

  function load(who: string): PendingRecord | null {
    try {
      const raw = svc.storage()?.getItem(storeKey(who))
      if (!raw) return null
      const r = JSON.parse(raw) as Partial<PendingRecord> | null
      const digits = (v: unknown): v is string => typeof v === 'string' && /^\d{1,78}$/.test(v)
      const creator = r?.creator
      const ok =
        typeof r?.id === 'string' &&
        typeof r.amount === 'string' &&
        TIP_AMOUNT.test(r.amount) &&
        typeof r.until === 'number' &&
        r.until > svc.now() &&
        digits(r.wei) &&
        digits(r.fromBlock) &&
        digits(r.nonce) &&
        (r.hash === undefined || (typeof r.hash === 'string' && /^0x[0-9a-fA-F]{64}$/.test(r.hash))) &&
        creator != null &&
        typeof creator.address === 'string' &&
        /^0x[0-9a-f]{40}$/.test(creator.address)
      if (!ok) {
        save(who, null)
        return null
      }
      // Kept as stored, with what the panel draws reduced to plain strings and the watch to its own length at most.
      const valid = r as PendingRecord
      return {
        id: valid.id,
        amount: valid.amount,
        wei: valid.wei,
        fromBlock: valid.fromBlock,
        nonce: valid.nonce,
        ...(valid.hash ? { hash: valid.hash } : {}),
        until: Math.min(valid.until, svc.now() + WATCH_FOR_MS),
        creator: {
          address: creator.address,
          ...(typeof creator.name === 'string' ? { name: creator.name } : {}),
          ...(creator.claimed === true ? { claimed: true } : {}),
          ...(typeof creator.nameColor === 'string' && /^#[0-9a-fA-F]{3,8}$/.test(creator.nameColor) ? { nameColor: creator.nameColor } : {}),
          ...(typeof creator.face === 'string' && /^https?:\/\//.test(creator.face) ? { face: creator.face } : {})
        }
      }
    } catch {
      return null
    }
  }

  /** Registers a signed tip as the account's pending one, here and in storage. */
  function begin(who: string, record: PendingRecord, release: (() => void) | null): Pending {
    const p: Pending = { record, onSettle: null, settled: false, ...(release ? { release } : {}) }
    pending.set(who, p)
    save(who, record)
    return p
  }

  function settle(who: string, p: Pending, outcome: Outcome): void {
    if (p.settled) return
    p.settled = true
    if (pending.get(who) === p) pending.delete(who)
    // Only this tip's record: another tab may since have stored a newer one (where it has no tip lock).
    if (load(who)?.id === p.record.id) save(who, null)
    p.release?.()
    if (p.onSettle) p.onSettle(outcome)
    else if (outcome === 'sent') tipped()
  }

  async function forward(hash: string, login: StoredLogin): Promise<void> {
    for (const delay of [0, ...FORWARD_RETRY_MS]) {
      if (delay) await svc.sleep(delay)
      try {
        await svc.forward(hash, login)
        return
      } catch (e) {
        console.warn('[donate] tip forward failed', e)
        // The catalyst refused the claim itself: asking again changes nothing.
        if (e instanceof ForwardRefused) return
      }
    }
  }

  /** Its Transfer (from the tipper to the creator, of exactly its amount, since the block it was signed at), if mined. */
  async function findTransfer(who: string, record: PendingRecord, token: ContractData): Promise<string | null> {
    const found = (await svc.transfers(token, who, record.creator.address, BigInt(record.fromBlock)).catch(() => [])).find((t) => t.value === BigInt(record.wei))
    return found?.hash ?? null
  }

  /**
   * A tip whose outcome is unknown: never sent again. It is watched by its hash (when there is one) and by its own
   * Transfer (the relay may have re-sent it under another hash) until the record's end; the first answer settles it,
   * and the polls stop with it. A revert, or the relay's refusal, fails it only once the chain shows it unused
   * (`settleFailure`); no answer leaves it unknown.
   */
  function watch(who: string, p: Pending, token: ContractData, login: StoredLogin | null): void {
    const { record } = p
    const chainId = token.chainId as ChainId
    const sent = (hash: string): void => {
      settle(who, p, 'sent')
      if (login) void forward(hash, login)
    }
    const known = record.hash
    const byHash: Promise<void> = known
      ? svc.wait(chainId, known, Math.max(0, record.until - svc.now()), () => p.settled).then(
          () => sent(known),
          (e: unknown) => (isTxReverted(e) ? settleFailure(who, p, token, login) : undefined)
        )
      : Promise.resolve()
    const byTransfer = (async (): Promise<void> => {
      while (!p.settled && svc.now() < record.until) {
        await svc.sleep(WATCH_EVERY_MS)
        if (p.settled) return
        const hash = await findTransfer(who, record, token)
        if (hash) return sent(hash)
      }
    })()
    void Promise.all([byHash, byTransfer]).then(() => settle(who, p, 'unknown'))
  }

  /**
   * The relay refused the tip, or its known transaction reverted: failed only while the chain shows its signature
   * unused. The relay also refuses after a broadcast it lost, and may have re-sent the tip under another hash; if the
   * token's meta-transaction nonce has moved past the one the tip signed, the tip (or another meta-transaction of the
   * same nonce) went through, so it is looked for and never retried: TRY AGAIN would sign the next nonce and could
   * pay twice. A nonce that can't be read leaves the tip watched.
   */
  async function settleFailure(who: string, p: Pending, token: ContractData, login: StoredLogin | null): Promise<void> {
    const now = await svc.nonce(token, who).catch(() => null)
    if (p.settled) return
    if (now != null && now <= BigInt(p.record.nonce)) return settle(who, p, 'failed')
    const hash = await findTransfer(who, p.record, token)
    if (p.settled) return
    if (hash) {
      settle(who, p, 'sent')
      if (login) void forward(hash, login)
      return
    }
    p.onShowProcessing?.()
    watch(who, p, token, login)
  }

  async function capabilities(): Promise<DonateCapabilities> {
    const who = account()
    const enabled = await svc.enabled().catch(() => false)
    if (!enabled) return { v: DONATE_PROTOCOL, available: false, reason: 'disabled' }
    if (!who) return { v: DONATE_PROTOCOL, available: false, reason: 'signedOut' }
    if (!web3(who)) return { v: DONATE_PROTOCOL, available: false, reason: 'notWeb3' }
    return { v: DONATE_PROTOCOL, available: true }
  }

  function close(s: { id: number }): void {
    if (session !== s) return
    session = null
    deps.ui.hide()
  }

  /** `read`, or a refusal after `ms`: a read the panel waits on never holds the dialog for good. */
  function bounded<T>(read: Promise<T>, ms = LOAD_TIMEOUT_MS): Promise<T> {
    let timer: ReturnType<typeof setTimeout> | undefined
    const late = new Promise<never>((_, reject) => (timer = setTimeout(() => reject(new Error('Donate couldn’t load in time.')), ms)))
    return Promise.race([read, late]).finally(() => clearTimeout(timer))
  }

  async function open(request: DonateRequest): Promise<void> {
    if (session || deps.otherDialogOpen()) throw donateBusy()
    const s = { id: ++sessions }
    session = s
    const live = (): boolean => session === s
    try {
      await openSession(s, live, request)
    } catch (e) {
      // Whatever failed before the panel was up to the player, the dialog is never left claimed.
      close(s)
      if (session === s) session = null
      throw e
    }
  }

  async function openSession(s: { id: number }, live: () => boolean, request: DonateRequest): Promise<void> {
    const signedIn = account()
    if (!signedIn || !web3(signedIn) || !(await svc.enabled().catch(() => false))) throw new ShellError(DONATE_UNAVAILABLE, 'Donate is not available for this sign-in.')
    const who: string = signedIn
    const injected = deps.wallet?.provider(who) ?? null
    if (!injected) throw new ShellError(DONATE_UNAVAILABLE, 'No wallet here can sign for this sign-in.')
    const provider: Eip1193Provider = injected
    const chainId = svc.chainId()
    let token: ContractData
    try {
      token = manaToken(chainId)
    } catch {
      throw new ShellError(DONATE_UNAVAILABLE, 'MANA is not known on this network.')
    }

    const opening = { pointer: request.input === 'pointer' }
    const show = (view: DonateView, handlers: Partial<DonateHandlers> = {}): void => {
      if (live()) deps.ui.show(view, { onClose: () => close(s), ...handlers }, opening)
    }
    /** The tip is mined: the panel says so and the emote plays once it is closed; with the panel gone, at once. */
    const sent = (creator: DonateCreator): void => {
      if (!live()) return tipped()
      show(
        { step: 'sent', creator },
        {
          onClose: () => {
            close(s)
            tipped()
          }
        }
      )
    }
    /** Shows a pending tip and what it settles to, in this panel while it is up. */
    const follow = (p: Pending, retry: () => void): void => {
      const processing = (final = false): void => show({ step: 'processing', amount: p.record.amount, creator: p.record.creator, ...(final ? { final } : {}) })
      processing()
      p.onShowProcessing = () => processing()
      p.onSettle = (outcome) => {
        if (outcome === 'sent') return sent(p.record.creator)
        if (!live()) return
        if (outcome === 'failed') show({ step: 'failed' }, { onRetry: retry, onSupport: () => deps.openExternal(svc.supportUrl()) })
        else processing(true)
      }
    }

    const resume = (lock: (() => void) | null): Promise<void> => proceed(s, live, { who, provider, token, chainId, show, sent, follow, request }, lock)

    // A tip of this account still unsettled in this tab: show it, whatever place was asked, until it settles.
    const here = pending.get(who)
    if (here) return follow(here, () => close(s))
    // Another tab holds the account's tip lock (sending or watching a tip): say so, and go on once it lets go.
    const lock = await svc.claim(who)
    if (!lock && svc.locksAvailable()) {
      show({ step: 'elsewhere' })
      void svc.claim(who, true).then((waited) => {
        if (!live()) return waited?.()
        resume(waited).catch(() => close(s))
      })
      return
    }
    await resume(lock)
  }

  /** The rest of an opening, holding the account's tip lock (or none, without Web Locks). */
  async function proceed(
    s: { id: number },
    live: () => boolean,
    ctx: {
      who: string
      provider: Eip1193Provider
      token: ContractData
      chainId: ChainId
      show: (view: DonateView, handlers?: Partial<DonateHandlers>) => void
      sent: (creator: DonateCreator) => void
      follow: (p: Pending, retry: () => void) => void
      request: DonateRequest
    },
    lock: (() => void) | null
  ): Promise<void> {
    const { who, provider, token, chainId, show, sent, follow, request } = ctx
    // A tip of this account left unsettled by another tab or before a reload: watch and show it, holding the lock.
    const record = load(who)
    if (record) {
      const p = begin(who, record, lock)
      watch(who, p, token, deps.login())
      return follow(p, () => close(s))
    }
    lock?.()

    show({ step: 'loading' })

    // Everything the panel shows, read here (each read bounded). A failure closes the panel.
    const target = await bounded(svc.tipTarget())
    // When the payments config can be read, it must name this build's chain and token, or nothing is sent.
    if (target && (target.chainId !== chainId || lower(target.manaToken) !== lower(token.address))) throw new Error('The deployment pays tips on another network.')
    const place = await bounded(svc.place(request.place))
    if (!place) throw new Error('This place has no creator to tip.')
    const [creator, balanceWei, usdRate, presets] = await bounded(
      Promise.all([svc.creator(place.creator), svc.balance(who, token), svc.usdRate().catch(() => null), svc.presets().catch(() => DEFAULT_PRESETS)])
    )
    let form: Extract<DonateView, { step: 'form' }> = { step: 'form', scene: place.title, creator, balance: weiToMana(balanceWei), presets, usdRate }

    /** The form again, with the amount the player last chose and, when a send was refused, why. */
    const toForm = (amount?: string, error?: string): void =>
      show({ ...form, ...(amount ? { amount } : {}), ...(error ? { error } : {}) }, {
        onSend: (a) => void send(a),
        onBuyMana: () => deps.openExternal(svc.accountUrl())
      })
    const failed = (amount: string): void =>
      show({ step: 'failed' }, {
        onRetry: () => toForm(amount),
        onSupport: () => deps.openExternal(svc.supportUrl())
      })

    let sending = false
    async function send(amount: string): Promise<void> {
      if (!live() || sending) return
      const wei = manaToWei(amount)
      const login = deps.login()
      if (!login || account() !== who) return toForm(amount, 'Your sign-in changed. Close Donate and open it again.')
      if (wei == null || !TIP_AMOUNT.test(amount) || wei < BigInt(MIN_TIP) * 10n ** 18n) return toForm(amount, `Tips must be ≥ ${MIN_TIP} MANA`)
      sending = true
      // One tab at a time sends the account's tip: the lock is held from here, and by the tip once it is signed.
      const tipLock = await svc.claim(who)
      const owned = { lock: tipLock }
      try {
        if (!tipLock && svc.locksAvailable()) return toForm(amount, 'A tip is being sent from another tab. Try again once it finishes.')
        // Closing now cancels: nothing was signed. After the signature the tip is pending and goes on without the panel.
        let cancelled = false
        show({ step: 'sending', amount, creator: form.creator }, { onClose: () => ((cancelled = true), close(s)) })
        // The balance now, not when the panel opened; and the block the tip's Transfer is looked for from.
        const [before, fromBlock] = await Promise.all([svc.balance(who, token).catch(() => null), svc.blockNumber(chainId).catch(() => null)])
        if (cancelled) return
        if (before == null || fromBlock == null) return toForm(amount, 'Couldn’t read your MANA balance. Try again.')
        form = { ...form, balance: weiToMana(before) }
        if (wei > before) return toForm(amount, 'Insufficient MANA')
        await sendTip(amount, wei, fromBlock, login, () => cancelled, owned)
      } finally {
        sending = false
        // Not handed to a signed tip: nothing is pending, so the lock goes now.
        owned.lock?.()
      }
    }

    async function sendTip(amount: string, wei: bigint, fromBlock: bigint, login: StoredLogin, cancelled: () => boolean, owned: { lock: (() => void) | null }): Promise<void> {
      // What left the wallet decides what a failure means: before the signature did, nothing can have reached the
      // chain. Tracked on the real wallet, under the guard: a request the guard refused never reached it. The tip is
      // pending from the moment it is signed, with the nonce it signed.
      let tip: Pending | null = null
      let posted = false
      const intent: DonationIntent = {
        from: who,
        chainId,
        token: { address: lower(token.address), name: token.name, version: token.version },
        to: form.creator.address,
        wei,
        // A close cancels only until the tip is signed; after that it goes on without the panel.
        current: () => (tip != null || !cancelled()) && account() === who
      }
      const tracked: Eip1193Provider = {
        ...provider,
        async request(args) {
          const answer = await provider.request(args)
          if (args.method === 'eth_signTypedData_v4') {
            // Closed while the wallet asked: the signature is never used.
            if (cancelled()) throw new Error('Donate was closed.')
            const nonce = signedNonce(args.params)
            if (nonce == null) throw new Error('Donate couldn’t read what was signed.')
            tip = begin(who, { id: svc.newId(), amount, creator: form.creator, wei: wei.toString(), fromBlock: fromBlock.toString(), nonce, until: svc.now() + WATCH_FOR_MS }, owned.lock)
            // The signed tip holds the lock now, until it settles.
            owned.lock = null
            const p = tip
            p.onShowProcessing = () => show({ step: 'processing', amount, creator: form.creator })
            p.onSettle = (outcome) => {
              if (outcome === 'sent') return sent(form.creator)
              if (!live()) return
              if (outcome === 'failed') failed(amount)
              else show({ step: 'processing', amount, creator: form.creator, final: true })
            }
          }
          return answer
        }
      }
      const wallet = svc.guard(tracked, intent)
      // The relay POST: timed out, and known to have been made (before it nothing can have been broadcast).
      const relayFetch: typeof fetch = (input, init) => {
        posted = true
        return fetch(input, { ...init, signal: AbortSignal.timeout(RELAY_TIMEOUT_MS) })
      }
      const signed = (): Pending | null => tip

      let hash: string
      try {
        hash = await svc.sendGasless(wallet, token, encodeManaTransfer(intent.to, wei), relayFetch)
      } catch (e) {
        const p = signed()
        if (!p) return cancelled() ? undefined : failed(amount)
        // Signed but never handed to the relay (the signature unusable, say): nothing was sent.
        if (!posted) return settle(who, p, 'failed')
        // Refused by the relay: failed only once the chain shows the signature unused.
        if (refusedByRelay(e)) return void settleFailure(who, p, token, login)
        p.onShowProcessing?.()
        return watch(who, p, token, login)
      }
      const p = signed()
      if (!p) return
      p.record.hash = hash
      save(who, p.record)
      try {
        await svc.wait(chainId, hash, RECEIPT_WAIT_MS, () => p.settled)
      } catch (e) {
        if (isTxReverted(e)) return void settleFailure(who, p, token, login)
        p.onShowProcessing?.()
        return watch(who, p, token, login)
      }
      settle(who, p, 'sent')
      void forward(hash, login)
    }

    toForm()
  }

  return {
    async request(method, params) {
      switch (method) {
        case DONATE_METHODS.capabilities:
          return capabilities()
        case DONATE_METHODS.target: {
          const place = parseDonateTarget(params)
          const key = JSON.stringify(place)
          const now = svc.now()
          const known = targets.get(key)
          if (known && now - known.at < TARGET_TTL_MS) return { v: DONATE_PROTOCOL, tippable: known.tippable }
          let found: ResolvedPlace | null
          try {
            found = await svc.place(place)
          } catch {
            // A failed read is not an answer: Donate stays hidden now and the next look asks again.
            return { v: DONATE_PROTOCOL, tippable: false }
          }
          for (const [k, v] of targets) if (now - v.at >= TARGET_TTL_MS) targets.delete(k)
          const tippable = found != null
          targets.set(key, { at: now, tippable })
          // A place covers several parcels: walking across it reads it once.
          for (const pos of found?.positions ?? []) {
            const [x, y] = pos.split(',').map(Number)
            if (Number.isInteger(x) && Number.isInteger(y)) targets.set(JSON.stringify({ parcel: { x, y } }), { at: now, tippable })
          }
          return { v: DONATE_PROTOCOL, tippable }
        }
        case DONATE_METHODS.open:
          await open(parseDonateRequest(params))
          return { v: DONATE_PROTOCOL }
        default:
          throw new Error(`unknown donate method: ${method}`)
      }
    },
    dialogOpen: () => session != null
  }
}
