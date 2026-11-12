// The web shell's side of Donate (lib/donateProtocol). The HUD names a place; this resolves the place's creator
// itself (the places service), reads the creator's profile, the player's MANA balance, the MANA price and the
// suggested amounts, and shows the player the panel over the app (./overlay). On SEND it transfers exactly the
// confirmed MANA to that creator with the wallet the host injected, bound to that tip (./guard): a gasless
// meta-transaction through the relay first, and a transaction from the wallet only when the relay clearly refused
// before broadcasting. An outcome it can't tell (the relay's answer lost, or the transaction not yet mined) is
// shown as processing and watched, never retried: a second try could pay twice. Once the tip is mined, the shell
// forwards its hash to the catalyst, signed by the login, which then tells the creator (`tip_received`), and posts
// `bevy-shell:donate` so the app plays the tip emote.
//
// Donate is offered to a web3 wallet sign-in only (AGENTS.md: MANA is shown to logged-in web3 wallets), and that is
// checked again here on every open, not only in the capabilities the HUD reads.

import { encodeFunctionData, parseAbi } from 'viem'
import type { Eip1193Provider } from '../../../data/auth/wallet'
import { catalystBase, getJSON } from '../../../data/catalyst/client'
import { fetchProfile, mapProfile, profileFaceUrl } from '../../../data/catalyst/profile'
import { fetchTipTarget, manaToWei, type TipTarget } from '../../../explorer/components/manaTip'
import { settlementChainId } from '../../../shop/checkout/chain/config'
import { flagsService } from '../../../shop/discovery/lib/remoteFlags'
import { parseRemoteFlags } from '@shop/discovery/lib/rolloutFlags'
import { MANA_TOKEN, type ContractData } from '@shop/market/lib/contracts'
import { MetaTransactionError, sendMetaTransaction } from '../../../shop/market/lib/metaTx'
import { rpc } from '../../../shop/market/lib/rpc'
import { declined, isTxReverted, sendCall, waitForReceipt } from '../../../shop/market/lib/tx'
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
import type { AuthIdentity as SignerIdentity } from '../../../data/auth/identity'
import { identitySigner } from '../../../shop/api/host'
import type { ShopSigner } from '@shop/api/signer'
import { guardedDonationProvider, TRANSFER_ABI, type DonationIntent } from './guard'

/** The suggested amounts when the flags file names none. */
export const DEFAULT_PRESETS = [166, 333, 500]
const DONATIONS_FLAG = 'explorer-alfa-donations'
const PRESETS_FLAG = 'explorer-alfa-recommended-donation-amount'
const BALANCE_ABI = parseAbi(['function balanceOf(address owner) view returns (uint256)'])
const RATE_CACHE_MS = 30 * 60_000
/** How long the panel waits for the tip to be mined before it says the tip is processing. */
const RECEIPT_WAIT_MS = 3 * 60_000
/** How long a processing tip is still watched (by its effect on the balance) after the panel says so. */
const WATCH_FOR_MS = 30 * 60_000
const WATCH_EVERY_MS = 10_000
const FORWARD_RETRY_MS = [5_000, 10_000, 20_000, 40_000, 60_000]
const TARGET_TTL_MS = 5 * 60_000
const FORWARD_TIMEOUT_MS = 10_000

/** The creator as the panel shows them: always the address, and the profile when it could be read. */
export interface DonateCreator {
  address: string
  name?: string
  claimed?: boolean
  nameColor?: string
  face?: string
}

export type DonateView =
  | { step: 'loading' }
  | { step: 'form'; scene: string; creator: DonateCreator; balance: number; presets: number[]; usdRate: number | null; error?: string; amount?: string }
  | { step: 'sending'; amount: string; creator: DonateCreator }
  | { step: 'processing'; amount: string; creator: DonateCreator; final?: boolean }
  | { step: 'sent'; creator: DonateCreator }
  | { step: 'failed' }

export interface DonateHandlers {
  onClose(): void
  /** The amount in MANA, as typed or picked ("166", "12.5"). */
  onSend?(amount: string): void
  onBuyMana?(): void
  onRetry?(): void
  onSupport?(): void
}

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
  /** Where an unsettled tip is kept across reloads and tabs; null when this browser keeps nothing. */
  storage(): Storage | null
  sendDirect(provider: Eip1193Provider, token: ContractData, to: string, wei: bigint): Promise<string>
  /** Resolves when mined; TxRevertedError on a revert, TxPendingError after `timeoutMs`. */
  wait(chainId: ChainId, hash: string, timeoutMs: number): Promise<string>
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
  async balance(account, token) {
    const data = encodeFunctionData({ abi: BALANCE_ABI, functionName: 'balanceOf', args: [account as `0x${string}`] })
    return BigInt(await rpc<string>(token.chainId, 'eth_call', [{ to: token.address, data }, 'latest']))
  },
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
  storage: () => {
    try {
      return window.localStorage
    } catch {
      return null
    }
  },
  sendDirect: (provider, token, to, wei) => sendCall(provider, { chainId: token.chainId, to: token.address, abi: TRANSFER_ABI, functionName: 'transfer', args: [to, wei] }),
  wait: (chainId, hash, timeoutMs) => waitForReceipt(chainId, hash, 'The tip', { timeoutMs }),
  async forward(hash, login) {
    const signer = loginSigner(login)
    if (!signer) throw new Error('The sign-in has expired.')
    const url = `${catalystBase()}/events-notifier/forward-blockchain`
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

/** The login as a request signer (as the Shop's purchases sign), or null when it has no key or has expired. */
function loginSigner(login: StoredLogin): ShopSigner | null {
  const key = login.identity.ephemeralIdentity.privateKey
  if (!key || loginExpired(login)) return null
  const { identity } = login
  return identitySigner({
    signer: rootAddress(identity),
    ephemeral: { address: identity.ephemeralIdentity.address, privateKey: key as `0x${string}` },
    expiration: String(identity.expiration),
    authChain: identity.authChain as SignerIdentity['authChain']
  })
}

/** A row of the places service's destinations, as far as Donate reads it. */
export interface DestinationRow {
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

/** A relay answer that clearly refused the tip before broadcasting it: only then may the wallet send it instead. */
function refusedBeforeBroadcast(e: unknown): boolean {
  return e instanceof MetaTransactionError && e.refused === true && typeof e.status === 'number' && e.status >= 400 && e.status < 500
}

/** How an unsettled tip ended: mined, reverted, or still unknown when the watch ran out. */
type Outcome = 'sent' | 'failed' | 'unknown'

/** A signed tip as kept across reloads and tabs, per account, until it settles or its watch ends. */
export interface PendingRecord {
  amount: string
  creator: DonateCreator
  /** The tip, in wei. */
  wei: string
  /** The balance just before signing, in wei: the baseline the tip's effect is measured from. */
  before: string
  /** Its transaction hash, once the relay or the wallet gave one. */
  hash?: string
  /** When the watch ends, in ms. */
  until: number
}

/** A tip signed but not yet settled. The panel showing it (if any) sets `onSettle`. */
interface Pending {
  record: PendingRecord
  onSettle: ((outcome: Outcome) => void) | null
  settled: boolean
}

const RELAY_TIMEOUT_MS = 60_000
const storeKey = (account: string): string => `dcl:donate:pending:v1:${account}`

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
      const r = JSON.parse(raw) as PendingRecord
      if (typeof r?.until !== 'number' || r.until <= svc.now() || typeof r.wei !== 'string' || typeof r.before !== 'string' || !r.creator?.address) {
        save(who, null)
        return null
      }
      return r
    } catch {
      return null
    }
  }

  /** Registers a signed tip as the account's pending one, here and in storage. */
  function begin(who: string, record: PendingRecord): Pending {
    const p: Pending = { record, onSettle: null, settled: false }
    pending.set(who, p)
    save(who, record)
    return p
  }

  function settle(who: string, p: Pending, outcome: Outcome): void {
    if (p.settled) return
    p.settled = true
    if (pending.get(who) === p) {
      pending.delete(who)
      save(who, null)
    }
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

  /**
   * A tip whose outcome is unknown: never sent again. It is watched by its hash (when there is one) and by its
   * effect on the balance (the relay may have re-sent it under another hash) until the record's end; the first
   * answer settles it. A revert fails it at once; no answer leaves it unknown.
   */
  function watch(who: string, p: Pending, token: ContractData, login: StoredLogin | null): void {
    const { record } = p
    const wei = BigInt(record.wei)
    const before = BigInt(record.before)
    const chainId = token.chainId as ChainId
    const byHash: Promise<void> = record.hash
      ? svc.wait(chainId, record.hash, Math.max(0, record.until - svc.now())).then(
          () => {
            settle(who, p, 'sent')
            if (login) void forward(record.hash!, login)
          },
          (e: unknown) => {
            if (isTxReverted(e)) settle(who, p, 'failed')
          }
        )
      : Promise.resolve()
    const byBalance = (async (): Promise<void> => {
      while (!p.settled && svc.now() < record.until) {
        await svc.sleep(WATCH_EVERY_MS)
        if (p.settled) return
        const now = await svc.balance(who, token).catch(() => null)
        if (now != null && before - now >= wei) return settle(who, p, 'sent')
      }
    })()
    void Promise.all([byHash, byBalance]).then(() => settle(who, p, 'unknown'))
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

  async function open(request: DonateRequest): Promise<void> {
    if (session || deps.otherDialogOpen()) throw donateBusy()
    const s = { id: ++sessions }
    session = s
    const live = (): boolean => session === s
    const release = (): void => {
      if (session === s) session = null
    }
    const who = account()
    let provider: Eip1193Provider | null = null
    let token: ContractData | undefined
    const chainId = svc.chainId()
    try {
      if (!who || !web3(who) || !(await svc.enabled().catch(() => false))) throw new ShellError(DONATE_UNAVAILABLE, 'Donate is not available for this sign-in.')
      provider = deps.wallet?.provider(who) ?? null
      if (!provider) throw new ShellError(DONATE_UNAVAILABLE, 'No wallet here can sign for this sign-in.')
      token = MANA_TOKEN[chainId]
      if (!token) throw new ShellError(DONATE_UNAVAILABLE, 'MANA is not known on this network.')
    } catch (e) {
      release()
      throw e
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
      show({ step: 'processing', amount: p.record.amount, creator: p.record.creator })
      p.onSettle = (outcome) => {
        if (outcome === 'sent') return sent(p.record.creator)
        if (!live()) return
        if (outcome === 'failed') show({ step: 'failed' }, { onRetry: retry, onSupport: () => deps.openExternal(svc.supportUrl()) })
        else show({ step: 'processing', amount: p.record.amount, creator: p.record.creator, final: true })
      }
    }

    // A tip of this account still unsettled, in this tab or (from storage) another one or before a reload: show it,
    // whatever place was asked, until it settles.
    let unsettled = pending.get(who)
    if (!unsettled) {
      const record = load(who)
      if (record) {
        unsettled = begin(who, record)
        watch(who, unsettled, token, deps.login())
      }
    }
    if (unsettled) {
      follow(unsettled, () => close(s))
      return
    }

    show({ step: 'loading' })

    // Everything the panel shows, read here. A failure closes the panel.
    let form: Extract<DonateView, { step: 'form' }>
    try {
      const target = await svc.tipTarget()
      // When the payments config can be read, it must name this build's chain and token, or nothing is sent.
      if (target && (target.chainId !== chainId || lower(target.manaToken) !== lower(token.address))) throw new Error('The deployment pays tips on another network.')
      const place = await svc.place(request.place)
      if (!place) throw new Error('This place has no creator to tip.')
      const [creator, wei, usdRate, presets] = await Promise.all([svc.creator(place.creator), svc.balance(who, token), svc.usdRate().catch(() => null), svc.presets().catch(() => DEFAULT_PRESETS)])
      form = { step: 'form', scene: place.title, creator, balance: weiToMana(wei), presets, usdRate }
    } catch (e) {
      close(s)
      release()
      throw e
    }

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
      try {
        // Closing now cancels: nothing was signed. After the signature the tip is pending and goes on without the panel.
        let cancelled = false
        show({ step: 'sending', amount, creator: form.creator }, { onClose: () => ((cancelled = true), close(s)) })
        // The balance now, not when the panel opened: the baseline the tip's effect is measured from.
        const before = await svc.balance(who!, token!).catch(() => null)
        if (cancelled) return
        if (before == null) return toForm(amount, 'Couldn’t read your MANA balance. Try again.')
        form = { ...form, balance: weiToMana(before) }
        if (wei > before) return toForm(amount, 'Insufficient MANA')
        await sendTip(amount, wei, before, login, () => cancelled)
      } finally {
        sending = false
      }
    }

    async function sendTip(amount: string, wei: bigint, before: bigint, login: StoredLogin, cancelled: () => boolean): Promise<void> {
      const intent: DonationIntent = {
        from: who!,
        chainId,
        token: { address: lower(token!.address), name: token!.name, version: token!.version },
        to: form.creator.address,
        wei,
        current: () => !cancelled() && account() === who
      }
      // What left the wallet decides what a failure means: before a signature or a transaction did, nothing can have
      // reached the chain; after, a failure that isn't an explicit refusal may have. Tracked on the real wallet, under
      // the guard: a request the guard refused never reached it. The tip is pending from the moment it is signed.
      let p: Pending | null = null
      let submitted = false
      let posted = false
      const tracked: Eip1193Provider = {
        ...provider!,
        async request(args) {
          if (args.method === 'eth_sendTransaction') {
            if (cancelled()) throw new Error('Donate was closed.')
            submitted = true
          }
          const answer = await provider!.request(args)
          if (args.method === 'eth_signTypedData_v4') {
            // Closed while the wallet asked: the signature is never used.
            if (cancelled()) throw new Error('Donate was closed.')
            p = begin(who!, { amount, creator: form.creator, wei: wei.toString(), before: before.toString(), until: svc.now() + WATCH_FOR_MS })
            const settledHere = p
            settledHere.onSettle = (outcome) => {
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
      const data = encodeFunctionData({ abi: TRANSFER_ABI, functionName: 'transfer', args: [intent.to as `0x${string}`, wei] })
      // The relay POST: timed out, and known to have been made (before it nothing can have been broadcast).
      const relayFetch: typeof fetch = (input, init) => {
        posted = true
        return fetch(input, { ...init, signal: AbortSignal.timeout(RELAY_TIMEOUT_MS) })
      }
      const pendingTip = (): Pending | null => p
      const processing = (): void => {
        const tip = pendingTip()
        if (!tip) return failed(amount)
        if (live()) show({ step: 'processing', amount, creator: form.creator })
        watch(who!, tip, token!, login)
      }

      let hash: string
      try {
        hash = await svc.sendGasless(wallet, token!, data, relayFetch)
      } catch (e) {
        const tip = pendingTip()
        if (!tip) return cancelled() ? undefined : failed(amount)
        // Signed but never handed to the relay (the signature unusable, say): nothing was sent.
        if (!posted) return settle(who!, tip, 'failed')
        if (!refusedBeforeBroadcast(e)) return processing()
        // The relay clearly refused before broadcasting: the wallet may send it itself.
        try {
          hash = await svc.sendDirect(wallet, token!, intent.to, wei)
        } catch (direct) {
          return submitted && !declined(direct) ? processing() : settle(who!, tip, 'failed')
        }
      }
      const tip = pendingTip()!
      tip.record.hash = hash
      save(who!, tip.record)
      try {
        await svc.wait(chainId, hash, RECEIPT_WAIT_MS)
      } catch (e) {
        if (isTxReverted(e)) return settle(who!, tip, 'failed')
        return processing()
      }
      settle(who!, tip, 'sent')
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
