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
import { declined, isTxPending, isTxReverted, sendCall, waitForReceipt } from '../../../shop/market/lib/tx'
import type { ChainId } from '@shop/model/chain'
import { loginExpired, rootAddress, type StoredLogin } from '@explorer-core/features/auth/sso'
import { siteUrl } from '../../lib/baseDomain'
import {
  DONATE_EVENT,
  DONATE_METHODS,
  DONATE_PROTOCOL,
  DONATE_UNAVAILABLE,
  MIN_TIP,
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
import { guardedDonationProvider, type DonationIntent } from './guard'

/** The suggested amounts when the flags file names none. */
export const DEFAULT_PRESETS = [166, 333, 500]
const DONATIONS_FLAG = 'explorer-alfa-donations'
const PRESETS_FLAG = 'explorer-alfa-recommended-donation-amount'
const BALANCE_ABI = parseAbi(['function balanceOf(address owner) view returns (uint256)'])
const TRANSFER_ABI = parseAbi(['function transfer(address to, uint256 amount) returns (bool)'])
const RATE_CACHE_MS = 30 * 60_000
/** How long the panel waits for the tip to be mined before it says the tip is processing. */
const RECEIPT_WAIT_MS = 3 * 60_000
/** How long a processing tip is still watched (by its effect on the balance) after the panel says so. */
const WATCH_FOR_MS = 30 * 60_000
const WATCH_EVERY_MS = 10_000
const FORWARD_RETRY_MS = [5_000, 10_000, 20_000, 40_000, 60_000]
const TARGET_TTL_MS = 5 * 60_000

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
  | { step: 'form'; scene: string; creator: DonateCreator; balance: number; presets: number[]; usdRate: number | null }
  | { step: 'sending'; amount: string; creator: DonateCreator }
  | { step: 'processing'; amount: string; creator: DonateCreator }
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
  sendGasless(provider: Eip1193Provider, token: ContractData, data: `0x${string}`): Promise<string>
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
  return values.length >= 1 ? values : DEFAULT_PRESETS
}

/** MANA in wei as a number of MANA for display. */
export function weiToMana(wei: bigint): number {
  return Number(wei / 10n ** 14n) / 10_000
}

const FLAGS_TTL_MS = 60_000
let flagsCache: { at: number; read: Promise<ReturnType<typeof parseRemoteFlags>> } | null = null

/** The Explorer's flags file (the same service the Shop reads its own from), kept a minute; a failed read is not kept. */
function readExplorerFlags(): Promise<ReturnType<typeof parseRemoteFlags>> {
  if (flagsCache && Date.now() - flagsCache.at < FLAGS_TTL_MS) return flagsCache.read
  const read = fetch(`${flagsService()}/explorer.json`).then(async (res) => {
    if (!res.ok) throw new Error(`feature flags: HTTP ${res.status}`)
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
    const res = await fetch(`${catalystBase()}/price/api/v3/simple/price?ids=decentraland&vs_currencies=usd`)
    const body = (await res.json().catch(() => null)) as { decentraland?: { usd?: unknown } } | null
    const rate = body?.decentraland?.usd
    if (!res.ok || typeof rate !== 'number' || !(rate > 0)) return null
    rateCache = { at: Date.now(), rate }
    return rate
  },
  tipTarget: () => fetchTipTarget().catch(() => null),
  chainId: () => settlementChainId(),
  guard: guardedDonationProvider,
  sendGasless: (provider, token, data) => sendMetaTransaction({ provider, contract: token, kind: 'collection', functionData: data }),
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
      body: JSON.stringify({ type: 'tip-received', transactionHash: hash })
    })
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

/** A relay answer that clearly refused the tip before broadcasting it: only then may the wallet send it instead. */
function refusedBeforeBroadcast(e: unknown): boolean {
  return e instanceof MetaTransactionError && e.refused === true && typeof e.status === 'number' && e.status >= 400 && e.status < 500
}

export function createShellDonate(deps: ShellDonateDeps): ShellDonate {
  const svc: ShellDonateServices = { ...LIVE, ...deps.services }
  /** The open panel, or null. Each opening is its own session: a late answer for an older one draws nothing. */
  let session: { id: number } | null = null
  let sessions = 0
  /** Places looked up for donate.target, kept a while so walking around reads each once. */
  const targets = new Map<string, { at: number; tippable: boolean }>()

  const account = (): string | null => {
    const login = deps.login()
    return login && !loginExpired(login) ? lower(rootAddress(login.identity)) : null
  }
  const web3 = (who: string): boolean => deps.wallet?.marketplace?.(who) === true && deps.wallet.kind(who) === 'web3'

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
    const who = account()
    if (!who || !web3(who) || !(await svc.enabled().catch(() => false))) throw new ShellError(DONATE_UNAVAILABLE, 'Donate is not available for this sign-in.')
    const provider = deps.wallet?.provider(who) ?? null
    if (!provider) throw new ShellError(DONATE_UNAVAILABLE, 'No wallet here can sign for this sign-in.')
    const chainId = svc.chainId()
    const token = MANA_TOKEN[chainId]
    if (!token) throw new ShellError(DONATE_UNAVAILABLE, 'MANA is not known on this network.')

    const s = { id: ++sessions }
    session = s
    const opening = { pointer: request.input === 'pointer' }
    const live = (): boolean => session === s
    const show = (view: DonateView, handlers: Partial<DonateHandlers> = {}): void => {
      if (live()) deps.ui.show(view, { onClose: () => close(s), ...handlers }, opening)
    }
    show({ step: 'loading' })

    // Everything the panel shows, read here. A failure closes the panel.
    let form: Extract<DonateView, { step: 'form' }>
    let balance: bigint
    try {
      const target = await svc.tipTarget()
      // The payments config and this build must agree on the chain and the token, or nothing is sent.
      if (target && (target.chainId !== chainId || lower(target.manaToken) !== lower(token.address))) throw new Error('The deployment pays tips on another network.')
      const place = await svc.place(request.place)
      if (!place) throw new Error('This place has no creator to tip.')
      const [creator, wei, usdRate, presets] = await Promise.all([svc.creator(place.creator), svc.balance(who, token), svc.usdRate().catch(() => null), svc.presets().catch(() => DEFAULT_PRESETS)])
      balance = wei
      form = { step: 'form', scene: place.title, creator, balance: weiToMana(wei), presets, usdRate }
    } catch (e) {
      close(s)
      throw e
    }

    const toForm = (): void =>
      show(form, {
        onSend: (amount) => void send(amount),
        onBuyMana: () => deps.openExternal(svc.accountUrl())
      })
    const failed = (): void =>
      show({ step: 'failed' }, {
        onRetry: toForm,
        onSupport: () => deps.openExternal(svc.supportUrl())
      })
    const tipped = (): void => deps.post({ type: DONATE_EVENT, v: DONATE_PROTOCOL, state: 'sent' })
    /** The tip is mined: the panel says so and the emote plays once it is closed; with the panel gone, at once. */
    const sent = (): void => {
      if (!live()) return tipped()
      show(
        { step: 'sent', creator: form.creator },
        {
          onClose: () => {
            close(s)
            tipped()
          }
        }
      )
    }

    async function forward(hash: string, login: StoredLogin): Promise<void> {
      for (const delay of [0, ...FORWARD_RETRY_MS]) {
        if (delay) await svc.sleep(delay)
        try {
          await svc.forward(hash, login)
          return
        } catch (e) {
          console.warn('[donate] tip forward failed', e)
        }
      }
    }

    /** Whether the tip's balance effect is seen within the watch window: confirms a tip whatever hash carried it. */
    async function watchBalance(amountWei: bigint): Promise<boolean> {
      const until = svc.now() + WATCH_FOR_MS
      while (svc.now() < until) {
        await svc.sleep(WATCH_EVERY_MS)
        const now = await svc.balance(who!, token!).catch(() => null)
        if (now != null && balance - now >= amountWei) return true
      }
      return false
    }

    async function send(amount: string): Promise<void> {
      const wei = manaToWei(amount)
      const login = deps.login()
      if (!live() || !login || wei == null || wei < BigInt(MIN_TIP) * 10n ** 18n || wei > balance) return
      const intent: DonationIntent = {
        from: who!,
        chainId,
        token: { address: lower(token!.address), name: token!.name, version: token!.version },
        to: form.creator.address,
        wei,
        current: () => live() && account() === who
      }
      // What left the wallet decides what a failure means: before a signature or a transaction did, nothing can
      // have reached the chain; after, a failure that isn't an explicit refusal may have.
      // Tracked on the real wallet, under the guard: a request the guard refused never reached it.
      let signed = false
      let submitted = false
      const tracked: Eip1193Provider = {
        ...provider!,
        async request(args) {
          if (args.method === 'eth_sendTransaction') submitted = true
          const answer = await provider!.request(args)
          if (args.method === 'eth_signTypedData_v4') signed = true
          return answer
        }
      }
      const wallet = svc.guard(tracked, intent)
      const data = encodeFunctionData({ abi: TRANSFER_ABI, functionName: 'transfer', args: [intent.to as `0x${string}`, wei] })
      show({ step: 'sending', amount, creator: form.creator })

      let hash: string
      try {
        hash = await svc.sendGasless(wallet, token!, data)
      } catch (e) {
        if (!signed) return failed()
        if (!refusedBeforeBroadcast(e)) return processing(amount, wei)
        // The relay clearly refused before broadcasting: the wallet may send it itself.
        try {
          hash = await svc.sendDirect(wallet, token!, intent.to, wei)
        } catch (direct) {
          return submitted && !declined(direct) ? processing(amount, wei) : failed()
        }
      }
      try {
        await svc.wait(chainId, hash, RECEIPT_WAIT_MS)
      } catch (e) {
        if (isTxReverted(e)) return failed()
        return processing(amount, wei, isTxPending(e) ? hash : undefined, login)
      }
      sent()
      void forward(hash, login)
    }

    /**
     * A tip whose outcome is unknown: never sent again. It is watched by its hash (when there is one) and by its
     * effect on the balance (the relay may have re-sent it under another hash); the first to confirm it wins. When
     * neither does within the window, nothing was paid, and the panel says the tip failed.
     */
    function processing(amount: string, wei: bigint, hash?: string, login?: StoredLogin): void {
      show({ step: 'processing', amount, creator: form.creator })
      let settled = false
      const confirm = (): void => {
        if (settled) return
        settled = true
        sent()
      }
      const byHash =
        hash && login
          ? svc.wait(chainId, hash, WATCH_FOR_MS).then(
              () => {
                confirm()
                void forward(hash, login)
                return true
              },
              () => false
            )
          : Promise.resolve(false)
      const byBalance = watchBalance(wei).then((seen) => {
        if (seen) confirm()
        return seen
      })
      void Promise.all([byHash, byBalance]).then(([a, b]) => {
        if (!a && !b && !settled && live()) failed()
      })
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
          const known = targets.get(key)
          if (known && svc.now() - known.at < TARGET_TTL_MS) return { v: DONATE_PROTOCOL, tippable: known.tippable }
          const found = await svc.place(place).catch(() => null)
          const tippable = found != null
          const at = svc.now()
          targets.set(key, { at, tippable })
          // A place covers several parcels: walking across it reads it once.
          for (const p of found?.positions ?? []) {
            const [x, y] = p.split(',').map(Number)
            if (Number.isInteger(x) && Number.isInteger(y)) targets.set(JSON.stringify({ parcel: { x, y } }), { at, tippable })
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
