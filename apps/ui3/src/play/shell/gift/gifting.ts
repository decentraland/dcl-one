// The web shell's side of Gift (lib/giftProtocol). The HUD's picker names a token and a recipient; this checks both
// itself before anything is signed: the token is a Polygon collection item of the build's chain that the
// marketplace index knows, the login's account owns it on chain right now, and the recipient is another account.
// It resolves the item and the recipient's profile on its own and shows them over the app (./overlay) for the player
// to confirm there, where the wallet is, with SEND GIFT: nothing is signed before that press, which the app frame
// can't make. Then "Preparing Gift for" while the wallet signs, "Gift Sent" once the relay takes it, or the error with
// TRY AGAIN. It signs with the wallet the host injected, bound to that one transfer (./guard), as a gasless
// meta-transaction on the collection, and nothing else.
//
// A token can move only once, so a gift can never be sent twice. An outcome the shell can't tell (the relay's answer
// lost) is settled by the token's owner: a token that left the account was sent. A gift the relay refused is failed
// while the collection's meta-transaction nonce is still the one it signed, and TRY AGAIN signs that nonce again.
//
// A gift sent is kept per account (the browser's storage) with the token's `transferredAt` when it was sent, so the
// picker leaves it out until the owned catalog shows it gone (gift.pending).

import { encodeFunctionData } from 'viem'
import type { Eip1193Provider } from '../../../data/auth/wallet'
import { fetchProfile, mapProfile, profileFaceUrl } from '../../../data/catalyst/profile'
import { settlementChainId } from '../../../shop/checkout/chain/config'
import { readRemoteFlags } from '../../../shop/discovery/lib/remoteFlags'
import { fetchToken } from '../../../shop/market/lib/api'
import { collectionV2, ERC721_ABI, type ContractData } from '@shop/market/lib/contracts'
import { MetaTransactionError, readNonce, sendMetaTransaction } from '../../../shop/market/lib/metaTx'
import { readContract } from '../../../shop/market/lib/rpc'
import { declined } from '../../../shop/market/lib/tx'
import type { ChainId } from '@shop/model/chain'
import { loginExpired, rootAddress, type StoredLogin } from '@explorer-core/features/auth/sso'
import type { PendingGift } from '@explorer-core/hostContract'
import { ShellError } from '@explorer-core/lib/shellReply'
import { siteUrl } from '../../lib/baseDomain'
import {
  GIFT_EVENT,
  GIFT_METHODS,
  GIFT_PROTOCOL,
  GIFT_UNAVAILABLE,
  parseGiftRequest,
  parsePendingGift,
  type GiftCapabilities,
  type GiftEvent,
  type GiftOpenRequest
} from '../../lib/giftProtocol'
import type { ShellShopWallet } from '../shop/purchase'
import { pageLocks, webLock } from '../webLock'
import { guardedGiftProvider, type GiftIntent } from './guard'
import { isSafeImageUrl, safeCreator } from '../safeDisplay'
import { GIFT_STATUS_MANAGED, GIFT_STATUS_WEB3, type GiftTransferHandlers, type GiftTransferItem, type GiftTransferRecipient, type GiftTransferView } from '@hud/features/gifting/GiftTransferPanel'

const GIFTING_FLAG = 'explorer-alfa-gifting'
const FLAGS_TIMEOUT_MS = 3_000
/** The long-running hint shows after ten seconds of waiting for the wallet. */
const SLOW_AFTER_MS = 10_000
/** How long an unclear gift is watched for its token leaving the account. */
const WATCH_FOR_MS = 3 * 60_000
const WATCH_EVERY_MS = 10_000
/** A pending gift is forgotten after a week: by then the owned catalog shows it gone, or it never left. */
const PENDING_FOR_MS = 7 * 24 * 60 * 60_000
const PENDING_MAX = 200
const ZERO = '0x0000000000000000000000000000000000000000'
const NETWORK: Record<number, string> = { 137: 'matic', 80002: 'amoy' }

export interface ShellGiftUi {
  show(view: GiftTransferView, handlers: GiftTransferHandlers): void
  hide(): void
}

/** The token as the marketplace index knows it. */
export interface IndexedToken {
  contractAddress: string
  tokenId: string
  owner: string
  name: string
  image: string
  rarity?: string
  category: string
  chainId: number
}

export interface ShellGiftServices {
  enabled(): Promise<boolean>
  chainId(): ChainId
  token(contract: string, tokenId: string): Promise<IndexedToken | null>
  recipient(address: string): Promise<GiftTransferRecipient>
  ownerOf(chainId: ChainId, contract: string, tokenId: bigint): Promise<string>
  nonce(collection: ContractData, account: string): Promise<bigint>
  guard(provider: Eip1193Provider, intent: GiftIntent): Eip1193Provider
  sendGasless(provider: Eip1193Provider, collection: ContractData, data: `0x${string}`, nonce?: bigint): Promise<string>
  /** One tab at a time sends an account's gift; null while another holds it. */
  claim(account: string): Promise<(() => void) | null>
  /** Whether this browser has Web Locks: without them nothing proves one tab is the only one. */
  locksAvailable(): boolean
  storage(): Storage | null
  sleep(ms: number): Promise<void>
  now(): number
  activityUrl(): string
  supportUrl(): string
}

export interface ShellGiftDeps {
  login(): StoredLogin | null
  wallet: ShellShopWallet | null
  ui: ShellGiftUi
  /** To the app frame. */
  post(event: GiftEvent): void
  openExternal(url: string): void
  /** Whether another dialog (the Shop's, Donate's) is up over the app. */
  otherDialogOpen(): boolean
  services?: Partial<ShellGiftServices>
}

export interface ShellGift {
  /** Answers a `gift.*` request from the app frame. */
  request(method: string, params: unknown): Promise<unknown>
  /** Whether the gift's dialog is up. */
  dialogOpen(): boolean
}

const lower = (s: string): string => s.toLowerCase()

const LIVE: ShellGiftServices = {
  async enabled() {
    return (await readRemoteFlags(AbortSignal.timeout(FLAGS_TIMEOUT_MS), 'explorer')).flags[GIFTING_FLAG] === true
  },
  chainId: () => settlementChainId(),
  async token(contract, tokenId) {
    const t = await fetchToken(contract, tokenId).catch(() => null)
    return t ? { contractAddress: t.contractAddress, tokenId: t.tokenId, owner: t.owner, name: t.name, image: t.image, rarity: t.rarity, category: t.category, chainId: t.chainId } : null
  },
  async recipient(address) {
    const avatar = await fetchProfile(address).catch(() => null)
    if (!avatar) return { address }
    const p = mapProfile(avatar, address)
    return safeRecipient({ address, name: p.name, claimed: p.hasClaimedName, nameColor: p.nameColor, face: profileFaceUrl(avatar) ?? undefined })
  },
  async ownerOf(chainId, contract, tokenId) {
    // Decoded by the ABI: an empty or malformed answer throws instead of reading as some owner.
    const owner = await readContract({ chainId, address: contract, abi: ERC721_ABI, functionName: 'ownerOf', args: [tokenId] })
    if (typeof owner !== 'string' || !/^0x[0-9a-fA-F]{40}$/.test(owner)) throw new Error('ownerOf answered no address')
    return owner.toLowerCase()
  },
  nonce: (collection, account) => readNonce(collection, account),
  guard: guardedGiftProvider,
  sendGasless: (provider, collection, data, nonce) => sendMetaTransaction({ provider, contract: collection, kind: 'collection', functionData: data, nonce }),
  claim: (account) => webLock(pageLocks(), `dcl-gift:account:${account}`, false),
  locksAvailable: () => pageLocks() != null,
  storage: () => {
    try {
      return window.localStorage
    } catch {
      return null
    }
  },
  sleep: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
  now: () => Date.now(),
  activityUrl: () => siteUrl('/shop/activity'),
  supportUrl: () => siteUrl('/help')
}

/** The recipient as the panel may draw them (../safeDisplay); just the address when the profile has nothing safe. */
function safeRecipient(r: Partial<GiftTransferRecipient> & { address: string }): GiftTransferRecipient {
  const c = safeCreator(r)
  return c ? { address: c.address, ...(c.name ? { name: c.name } : {}), ...(c.claimed ? { claimed: true } : {}), ...(c.nameColor ? { nameColor: c.nameColor } : {}), ...(c.face ? { face: c.face } : {}) } : { address: r.address }
}

/** The item as the panel may draw it: an `https:` picture only. */
function safeItem(t: IndexedToken): GiftTransferItem {
  return {
    name: t.name,
    rarity: (t.rarity ?? 'base').toLowerCase(),
    category: t.category || undefined,
    ...(isSafeImageUrl(t.image) ? { image: t.image } : {})
  }
}

/** The token urn's network and contract, when it names a collection-v2 token. */
export function tokenParts(urn: string, tokenId: string): { network: string; contract: string } | null {
  const parts = urn.split(':')
  if (parts.length !== 7 || parts[3] !== 'collections-v2' || parts[6] !== tokenId || !/^0x[0-9a-fA-F]{40}$/.test(parts[4] ?? '')) return null
  return { network: parts[2]!, contract: parts[4]!.toLowerCase() }
}

const storeKey = (account: string): string => `dcl:gift:pending:v1:${account}`

interface StoredGift extends PendingGift {
  at: number
}

type Outcome = 'sent' | 'failed'
/** One opening of the gift's dialog: a late answer for an older one draws nothing; one run at a time. */
interface Session {
  id: number
  running: boolean
}

export function createShellGift(deps: ShellGiftDeps): ShellGift {
  const svc: ShellGiftServices = { ...LIVE, ...deps.services }
  /** The open dialog, or null. Each opening is its own session: a late answer for an older one draws nothing. */
  let session: Session | null = null
  let sessions = 0

  const account = (): string | null => {
    const login = deps.login()
    return login && !loginExpired(login) && !login.guest ? lower(rootAddress(login.identity)) : null
  }

  function load(who: string): StoredGift[] {
    try {
      const raw = svc.storage()?.getItem(storeKey(who))
      const list = raw ? (JSON.parse(raw) as unknown) : []
      if (!Array.isArray(list)) return []
      const now = svc.now()
      return list.flatMap((v) => {
        const g = parsePendingGift(v)
        const at = (v as { at?: unknown })?.at
        return g && typeof at === 'number' && at > now - PENDING_FOR_MS ? [{ ...g, at }] : []
      })
    } catch {
      return []
    }
  }

  function remember(who: string, gift: PendingGift): void {
    const list = [...load(who).filter((g) => g.urn !== gift.urn), { ...gift, at: svc.now() }].slice(-PENDING_MAX)
    try {
      svc.storage()?.setItem(storeKey(who), JSON.stringify(list))
    } catch {
      // Kept in this tab's memory only, through the event the app keeps.
    }
  }

  async function capabilities(): Promise<GiftCapabilities> {
    const login = deps.login()
    if (!login || loginExpired(login)) return { v: GIFT_PROTOCOL, available: false, reason: 'signedOut' }
    if (login.guest) return { v: GIFT_PROTOCOL, available: false, reason: 'guest' }
    const who = account()
    if (!who || !deps.wallet?.kind(who) || !deps.wallet.provider(who)) return { v: GIFT_PROTOCOL, available: false, reason: 'noWallet' }
    if (!(await svc.enabled().catch(() => false))) return { v: GIFT_PROTOCOL, available: false, reason: 'disabled' }
    return { v: GIFT_PROTOCOL, available: true }
  }

  const live = (s: Session): boolean => session === s

  function close(): void {
    session = null
    deps.ui.hide()
  }

  /** Checks the gift and resolves what the panel shows; a string names why it can't be sent. */
  async function check(req: GiftOpenRequest, who: string): Promise<{ recipient: GiftTransferRecipient; item: GiftTransferItem; contract: string; chain: ChainId } | string> {
    const chain = svc.chainId()
    const parts = tokenParts(req.urn, req.tokenId)
    if (!parts || parts.network !== NETWORK[chain]) return 'not a Polygon collection token of this chain'
    if (req.recipient === who || req.recipient === ZERO) return 'no recipient'
    const [token, recipient, owner] = await Promise.all([
      svc.token(parts.contract, req.tokenId).catch(() => null),
      svc.recipient(req.recipient).catch(() => ({ address: req.recipient })),
      svc.ownerOf(chain, parts.contract, BigInt(req.tokenId)).catch(() => null)
    ])
    if (!token || token.chainId !== chain || token.contractAddress.toLowerCase() !== parts.contract || token.tokenId !== req.tokenId) return 'not a token the marketplace index knows'
    if (owner !== who) return 'not owned by the sender'
    return { recipient: safeRecipient(recipient), item: safeItem(token), contract: parts.contract, chain }
  }

  async function run(req: GiftOpenRequest, who: string, s: Session, pinned?: bigint): Promise<void> {
    // TRY AGAIN pressed twice starts one run, never two.
    if (s.running) return
    s.running = true
    try {
      await attempt(req, who, s, pinned)
    } finally {
      s.running = false
    }
  }

  async function attempt(req: GiftOpenRequest, who: string, s: Session, pinned?: bigint): Promise<void> {
    const handlers = (retry: () => void, confirm: () => void = () => undefined): GiftTransferHandlers => ({
      onClose: () => live(s) && close(),
      onConfirm: confirm,
      onRetry: retry,
      onActivity: () => deps.openExternal(svc.activityUrl()),
      onSupport: () => deps.openExternal(svc.supportUrl())
    })
    let retryNonce: bigint | undefined
    const fail = (): void => {
      if (live(s)) deps.ui.show({ step: 'failed' }, handlers(() => live(s) && void run(req, who, s, retryNonce)))
    }
    deps.ui.show({ step: 'loading' }, handlers(() => undefined))
    const checked = await check(req, who)
    if (typeof checked === 'string') {
      console.warn('[gift] refused:', checked)
      fail()
      return
    }
    const { recipient, item, contract, chain } = checked
    // The player confirms here, in the shell, what the shell resolved: nothing is signed before SEND GIFT.
    const confirmed = await new Promise<boolean>((resolve) => {
      if (!live(s)) return resolve(false)
      deps.ui.show({ step: 'confirm', recipient, item }, { ...handlers(() => undefined, () => resolve(true)), onClose: () => (resolve(false), live(s) && close()) })
    })
    if (!confirmed || !live(s)) return
    const release = await svc.claim(who).catch(() => null)
    if (release == null && svc.locksAvailable()) {
      console.warn('[gift] another tab is sending a gift for this account')
      fail()
      return
    }
    const status = deps.wallet?.kind(who) === 'managed' ? GIFT_STATUS_MANAGED : GIFT_STATUS_WEB3
    const preparing = (slow: boolean): void => {
      if (live(s)) deps.ui.show({ step: 'preparing', recipient, item, status, slow }, handlers(() => undefined))
    }
    preparing(false)
    const slowTimer = setTimeout(() => preparing(true), SLOW_AFTER_MS)
    const collection = collectionV2(chain, contract)
    const tokenId = BigInt(req.tokenId)
    let signedNonce: bigint | undefined
    /** Whether the wallet returned a signature: only then can the gift have reached the relay. */
    let signed = false
    const settle = (outcome: Outcome): void => {
      clearTimeout(slowTimer)
      release?.()
      if (outcome === 'sent') {
        const gift = { urn: req.urn, transferredAt: req.transferredAt }
        remember(who, gift)
        deps.post({ type: GIFT_EVENT, v: GIFT_PROTOCOL, state: 'sent', ...gift })
        if (live(s)) deps.ui.show({ step: 'sent', recipient }, handlers(() => undefined))
      } else fail()
    }
    try {
      const provider = deps.wallet?.provider(who)
      if (!provider) throw new Error('no wallet')
      const intent: GiftIntent = {
        from: who,
        chainId: chain,
        collection: { address: contract, name: collection.name, version: collection.version },
        to: req.recipient,
        tokenId,
        ...(pinned !== undefined ? { nonce: pinned } : {}),
        // Closing the dialog cancels the gift until the wallet has signed: the guard refuses to sign once it is
        // closed, and drops a signature that comes back after it was closed.
        current: () => account() === who && live(s)
      }
      const checkedProvider = svc.guard(provider, intent)
      const guarded: Eip1193Provider = {
        ...checkedProvider,
        async request(args) {
          const result = await checkedProvider.request(args)
          if (args.method === 'eth_signTypedData_v4') signed = true
          return result
        }
      }
    // The nonce the wallet signs, read the way the sender reads it, so a refusal can tell whether it was used.
      signedNonce = pinned ?? (await svc.nonce(collection, who))
      const data = encodeFunctionData({ abi: ERC721_ABI, functionName: 'transferFrom', args: [who as `0x${string}`, req.recipient as `0x${string}`, tokenId] })
      await svc.sendGasless(guarded, collection, data, signedNonce)
      settle('sent')
    } catch (e) {
      // Nothing signed (declined, refused by the guard, no wallet, a read that failed, closed first): nothing can land.
      if (!signed || declined(e)) {
        console.warn('[gift] not signed:', e)
        settle('failed')
        return
      }
      settle(await resolveUnclear(e))
    }

    // The relay's answer was a refusal or was lost: the token's owner tells whether the gift went through.
    async function resolveUnclear(e: unknown): Promise<Outcome> {
      const refused = e instanceof MetaTransactionError && e.refused === true
      // A refusal is looked at once; a lost answer is watched for a while, by the clock and by count.
      const deadline = svc.now() + (refused ? 0 : WATCH_FOR_MS)
      const looks = refused ? 1 : Math.ceil(WATCH_FOR_MS / WATCH_EVERY_MS) + 1
      for (let look = 1; ; look++) {
        const owner = await svc.ownerOf(chain, contract, tokenId).catch(() => null)
        if (owner != null && owner !== who) return 'sent'
        if (look >= looks || svc.now() >= deadline) break
        await svc.sleep(WATCH_EVERY_MS)
      }
      // Still the sender's: TRY AGAIN signs the same nonce while it is unused, so the two can't both run.
      const now = await svc.nonce(collection, who).catch(() => null)
      retryNonce = signedNonce !== undefined && now === signedNonce ? signedNonce : undefined
      console.warn('[gift] not sent:', e)
      return 'failed'
    }
  }

  async function open(params: unknown): Promise<unknown> {
    const req = parseGiftRequest(params)
    const caps = await capabilities()
    const who = account()
    if (!caps.available || !who) throw new ShellError(GIFT_UNAVAILABLE, `Gift is unavailable (${caps.reason ?? 'unknown'})`)
    if (session != null || deps.otherDialogOpen()) throw new ShellError('gift.busy', 'a dialog is already open')
    const s: Session = { id: ++sessions, running: false }
    session = s
    void run(req, who, s)
    return { v: GIFT_PROTOCOL }
  }

  return {
    async request(method, params) {
      switch (method) {
        case GIFT_METHODS.capabilities:
          return capabilities()
        case GIFT_METHODS.pending: {
          const who = account()
          return { v: GIFT_PROTOCOL, pending: who ? load(who).map(({ urn, transferredAt }) => ({ urn, transferredAt })) : [] }
        }
        case GIFT_METHODS.open:
          return open(params)
        default:
          throw new Error(`unknown gift method ${method}`)
      }
    },
    dialogOpen: () => session != null
  }
}
