// The web shell's side of the in-world Shop purchase (lib/shopProtocol). The HUD asks to buy an item; this
// reads the item's listing, the server's offer and the buyer's Credits balance itself, shows the buyer the
// design's dialog over the app (./overlay), and on BUY ITEM settles the purchase through the web Shop's own
// checkout (cart partition, review, the per-group executor and its rails), with the wallet the host
// injected, bound to the confirmed purchase (./walletGuard). The buyer is the root signer of the login
// this shell holds; the HUD never names it.
//
// Every operation is kept per buyer (./ledger) with its own checkout, and its state goes to the HUD as
// `bevy-shell:shop` events and answers shop.status. A purchase handed off to the wallet or the relay is
// never failed or retried by a timeout: it stays pending and is watched (the executor's watchGroup: its
// receipt, or the server's record of its credit) until it settles, across dialogs and reloads. Buying the
// same item again while one is unresolved shows that one instead.

import type { AuthIdentity as SignerIdentity } from '../../../data/auth/identity'
import type { Eip1193Provider } from '../../../data/auth/wallet'
import { createShopClient, type ShopClient } from '../../../shop/api/shop'
import { purchaseOfferSource } from '../../../shop/api/offers'
import { identitySigner, type ShopSigner } from '../../../shop/api/signer'
import { partitionCart, type CartGroup } from '../../../shop/cart/partition'
import type { CartAddInput, CartStore } from '../../../shop/cart/store'
import { shopChainReader, type ChainReader } from '../../../shop/checkout/chain/rpc'
import { liveResolvers, reviewCart, type SettlementResolvers } from '../../../shop/checkout/purchase/cart'
import { releaseAuthorizing, runGroup, watchGroup, type CheckoutDeps, type GroupResult } from '../../../shop/checkout/purchase/executor'
import type { RailContext } from '../../../shop/checkout/purchase/rail'
import { createSpendGuard } from '../../../shop/checkout/purchase/spend-guard'
import { getUsdBalance, type UsdBalance } from '../../../shop/credits/credits'
import type { OfferSource } from '../../../shop/detail/offers'
import { fetchShopListingForItem } from '../../../shop/detail/api'
import type { CatalogItem } from '../../../shop/discovery/lib/catalogItem'
import { fetchRemoteFlags, primeRemoteFlags } from '../../../shop/discovery/lib/remoteFlags'
import { parseEvmAddress } from '../../../shop/model/address'
import { itemAsset } from '../../../shop/model/asset'
import { creditsCeil, creditsFloor, type UsdCents } from '../../../shop/model/money'
import type { CartLine, GroupProgress, GroupStatus, PurchaseOffer, PurchaseState } from '../../../shop/model/purchase'
import type { Session } from '../../../shop/model/settlement'
import { loginExpired, rootAddress, type StoredLogin } from '../../features/auth/sso'
import type { ShopPurchaseItem, ShopPurchaseStep } from '../../features/shop/ui'
import { siteUrl } from '../../lib/baseDomain'
import {
  parseBuyRequest,
  parseOperationRef,
  SHOP_EVENT,
  SHOP_METHODS,
  SHOP_PROTOCOL,
  TERMINAL_STATES,
  type ShopBalance,
  type ShopBuyRequest,
  type ShopCapabilities,
  type ShopOperation,
  type ShopOperationEvent,
  type ShopOperationState,
  type ShopWalletKind
} from '../../lib/shopProtocol'
import { withUtm } from '../../lib/utm'
import { trustedCreditsManager, type TrustedManager } from './config'
import { createLedger, type OperationRecord } from './ledger'
import { assertWalletAccount, guardedProvider, WalletRefusal, type PurchaseIntent } from './walletGuard'

/** The buyer's wallet, as the page hosting the shell resolves it (Sites: its sign-in session). */
export interface ShellShopWallet {
  /** How `account` signs here: an in-app (managed) wallet, a browser (web3) wallet, or neither. */
  kind(account: string): ShopWalletKind | null
  /** That wallet as an EIP-1193 provider for `account`, or null when it can't sign for it. */
  provider(account: string): Eip1193Provider | null
}

/** The dialog's item without its glyph: the overlay draws the category's. */
export type ShellPurchaseItem = Omit<ShopPurchaseItem, 'categoryIcon'> & { category?: string }
type WithItem<S> = S extends { item: ShopPurchaseItem } ? Omit<S, 'item'> & { item: ShellPurchaseItem } : S
export type ShellPurchaseView = WithItem<ShopPurchaseStep>

export interface ShellPurchaseHandlers {
  onClose: () => void
  onConfirm?: () => void
  onBuyCredits?: () => void
  onBackpack?: () => void
  onRetry?: () => void
  onSignIn?: () => void
}

/** Draws the dialog over the app, or takes it away. */
export interface ShellShopUi {
  /** `opening.pointer`: the dialog opens from a pointer press in the app, so its first focus isn't rung. */
  show(view: ShellPurchaseView, handlers: ShellPurchaseHandlers, opening?: { pointer: boolean }): void
  hide(): void
}

type Resolved = Parameters<typeof runGroup>[2]

/** What the purchase reads and runs, injectable for tests. */
export interface ShellShopServices {
  client(signer: () => ShopSigner | null): ShopClient
  fetchItem(contract: string, itemId: string, client: ShopClient): Promise<CatalogItem | null>
  offers(client: ShopClient): OfferSource
  balance(account: string, client: ShopClient): Promise<UsdBalance>
  resolvers(client: ShopClient): SettlementResolvers
  reader(): ChainReader
  /** The creator-sales rollout, which the listing reads from its module (see features/shop/flagsQuery). */
  primeFlags(): Promise<void>
  run(deps: CheckoutDeps, group: CartGroup, resolved: Resolved): Promise<GroupResult>
  watch(deps: CheckoutDeps, progress: GroupProgress): Promise<GroupResult>
  /** The CreditsManager this deployment trusts on a chain (./config). */
  manager(chainId: number): TrustedManager | null
  /** The wallet bound to one confirmed purchase (./walletGuard). */
  guard(provider: Eip1193Provider, intent: PurchaseIntent): Eip1193Provider
  /** The rails' relay and wait settings (tests point them at fakes). */
  rail(): Pick<RailContext, 'relayerUrl' | 'fetchImpl' | 'gasless' | 'wait'>
  storage(): Storage | null
  sleep(ms: number): Promise<void>
  now(): number
  newId(): string
  /** The web Shop's credit packs, for BUY CREDITS. */
  creditsUrl(): string
}

export interface ShellShopDeps {
  /** The login the shell holds and signs with (shell/login.ts), key included. */
  login(): StoredLogin | null
  wallet: ShellShopWallet | null
  ui: ShellShopUi
  /** To the app frame. */
  post(event: ShopOperationEvent): void
  /** Opens a page in a new tab (BUY CREDITS). */
  openExternal(url: string): void
  /** Leaves for the sign-in, as the HUD's own sign-in does. */
  signIn(): void
  services?: Partial<ShellShopServices>
}

const FLAGS_TIMEOUT_MS = 3000
/** How long a page keeps watching an unsettled purchase; a later page (a reload) carries on. */
const WATCH_FOR_MS = 30 * 60_000
const WATCH_BACKOFF_MS = [5_000, 10_000, 20_000, 40_000, 60_000]
/** How far each checkout state is along, to tell the newer of two copies when their clocks tie. */
const RANK: Record<GroupStatus, number> = { idle: 0, authorizing: 1, submitted: 2, pending: 3, confirmed: 4, failed: 4, expired: 4 }

function defaultStorage(): Storage | null {
  try {
    return typeof localStorage !== 'undefined' ? localStorage : null
  } catch {
    return null
  }
}

const LIVE: ShellShopServices = {
  client: (signer) => createShopClient({ signer }),
  fetchItem: (contract, itemId, client) => fetchShopListingForItem(contract, itemId, { client }),
  offers: (client) => purchaseOfferSource(client),
  balance: (account, client) => getUsdBalance(account, client),
  resolvers: (client) => liveResolvers(client),
  reader: () => shopChainReader(),
  primeFlags: async () => primeRemoteFlags(await fetchRemoteFlags(AbortSignal.timeout(FLAGS_TIMEOUT_MS))),
  run: runGroup,
  watch: watchGroup,
  manager: (chainId) => trustedCreditsManager(chainId),
  guard: guardedProvider,
  rail: () => ({}),
  storage: defaultStorage,
  sleep: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
  now: () => Date.now(),
  newId: () => crypto.randomUUID(),
  creditsUrl: () => withUtm(siteUrl('/shop/credits'))
}

/** A watcher signs nothing: the wallet is never asked while a purchase is only followed. */
const NO_WALLET: Eip1193Provider = {
  request: async () => {
    throw new WalletRefusal('Nothing is signed while a purchase is followed.', 4200)
  }
}

/** The shell's request signer for a login: its ephemeral key over its auth chain (ADR-44 headers). */
export function loginSigner(login: StoredLogin | null): ShopSigner | null {
  const key = login?.identity.ephemeralIdentity.privateKey
  if (!login || !key || loginExpired(login)) return null
  const { identity } = login
  return identitySigner({
    signer: rootAddress(identity),
    ephemeral: { address: identity.ephemeralIdentity.address, privateKey: key as `0x${string}` },
    expiration: String(identity.expiration),
    // The stored chain is the auth site's JSON; its link types are the signer's own.
    authChain: identity.authChain as SignerIdentity['authChain']
  })
}

/** One login, as signed: a new sign-in (or its renewal) is another, even for the same account. */
const loginKey = (login: StoredLogin): string => `${rootAddress(login.identity).toLowerCase()}|${login.identity.ephemeralIdentity.address.toLowerCase()}|${login.identity.expiration}`

/** The web Shop's cart input for a listing (discovery/components/AssetCard's cartInputFor), for an item. */
function cartInput(item: CatalogItem): CartAddInput | null {
  const contract = parseEvmAddress(item.contractAddress)
  if (!contract || item.itemId == null) return null
  let asset
  try {
    asset = itemAsset(item.chainId, contract, item.itemId)
  } catch {
    return null
  }
  return {
    asset,
    listing: { ...(item.tradeId ? { tradeId: item.tradeId } : {}), ...(item.acquisition ? { acquisition: item.acquisition } : {}) },
    display: {
      name: item.name,
      thumbnail: item.thumbnail || undefined,
      category: item.category,
      ...(item.wearableCategory ? { wearableCategory: item.wearableCategory } : {}),
      rarity: item.rarity,
      creator: item.creator || undefined,
      urn: item.urn
    },
    provenance: { source: 'in_world_shop' },
    available: item.available ?? null
  }
}

function shortAddress(address: string): string {
  return address.length > 10 ? `${address.slice(0, 6)}…${address.slice(-4)}` : address
}

/** What the buyer is shown agreeing to: the session it was read for and the quoted terms. */
interface Quote {
  login: string
  buyer: string
  kind: ShopWalletKind
  session: Session
  client: ShopClient
  line: CartLine
  offer: PurchaseOffer
  group: CartGroup
  item: ShellPurchaseItem
  balance: number
  /** Set when the wallet changed account or went away after the terms were shown. */
  stale?: string
}

/** An operation and, while its dialog is up, what it shows. */
type Operation = OperationRecord & { store?: CartStore; watching?: boolean; unwatchWallet?: () => void; input?: 'pointer' | 'keyboard' }

export interface ShellShop {
  /** Answers a `shop.*` request from the app frame. */
  request(method: string, params: unknown): Promise<unknown>
}

export function createShellShop(deps: ShellShopDeps): ShellShop {
  const svc: ShellShopServices = { ...LIVE, ...deps.services }
  const ledger = createLedger(svc.storage)
  const operations = new Map<string, Operation>()
  /** The operation whose dialog is up. */
  let dialog: Operation | null = null
  let restoredFor: string | null = null

  const snapshot = (op: Operation): ShopOperation => ({
    v: SHOP_PROTOCOL,
    operation: op.id,
    state: op.state,
    item: op.item,
    ...(op.next ? { next: op.next } : {}),
    ...(op.txHash !== undefined ? { txHash: op.txHash } : {})
  })
  const keep = (op: Operation): boolean => {
    op.updatedAt = svc.now()
    const { store: _s, watching: _w, unwatchWallet: _u, input: _i, ...record } = op
    return ledger.save(record)
  }
  const publish = (op: Operation): void => {
    keep(op)
    deps.post({ type: SHOP_EVENT, ...snapshot(op) })
  }
  /** A state the operation moves to; a settled one is final. */
  const move = (op: Operation, state: ShopOperationState, extra: Partial<Operation> = {}): void => {
    if (TERMINAL_STATES.includes(op.state)) return
    Object.assign(op, extra, { state })
    publish(op)
  }
  const show = (op: Operation, view: ShellPurchaseView, handlers: ShellPurchaseHandlers): void => {
    dialog = op
    deps.ui.show(view, handlers, { pointer: op.input === 'pointer' })
  }
  const hide = (op: Operation): void => {
    op.unwatchWallet?.()
    op.unwatchWallet = undefined
    if (dialog === op) {
      dialog = null
      deps.ui.hide()
    }
  }
  const close = (op: Operation, state: ShopOperationState): void => {
    hide(op)
    move(op, state)
  }

  const signedIn = (): { login: StoredLogin; buyer: string } | null => {
    const login = deps.login()
    if (!login || login.guest || loginExpired(login) || !login.identity.ephemeralIdentity.privateKey) return null
    return { login, buyer: rootAddress(login.identity).toLowerCase() }
  }
  const client = (): ShopClient => svc.client(() => loginSigner(deps.login()))

  const fail = (op: Operation, message: string, retry?: () => void): void => {
    show(op, { step: 'failed', message, retry: retry != null }, { onClose: () => close(op, 'failed'), ...(retry ? { onRetry: retry } : {}) })
  }
  const showPending = (op: Operation): void => show(op, { step: 'pending' }, { onClose: () => hide(op) })
  const showSuccess = (op: Operation): void => {
    if (!op.display) {
      hide(op)
      return
    }
    show(op, { step: 'success', item: op.display }, {
      onClose: () => hide(op),
      onBackpack: () => {
        hide(op)
        op.next = 'backpack'
        publish(op)
      }
    })
  }
  /** A watched purchase reached its end: its dialog, if it is the one up, says so. */
  const settled = (op: Operation, state: 'succeeded' | 'failed' | 'cancelled', extra: Partial<Operation> = {}, message?: string): void => {
    move(op, state, extra)
    if (dialog !== op) return
    if (state === 'succeeded') showSuccess(op)
    else if (state === 'failed') show(op, { step: 'failed', message: message ?? "This purchase didn't go through. Nothing was bought.", retry: false }, { onClose: () => hide(op) })
    else hide(op)
  }

  /**
   * Whether the purchase's progress is safely kept before the next step: no failed write for it, and its
   * record, read back from storage, holding the credit (`authorized`) or the hand-off (`handedOff`).
   */
  const durable = (op: Operation, stage: 'authorized' | 'handedOff'): boolean => {
    if (!op.buyer || ledger.failed(op.id)) return false
    const progress = ledger.saved(op.buyer, op.id)?.progress
    if (!progress || progress.creditIds.length === 0) return false
    return RANK[progress.status] >= (stage === 'authorized' ? RANK.authorizing : RANK.submitted) && progress.status !== 'failed' && progress.status !== 'expired'
  }

  // ---- Following a purchase that was handed off -------------------------------------------------

  const watchDeps = (op: Operation, store: CartStore): CheckoutDeps => ({
    ctx: { buyer: op.buyer!, provider: NO_WALLET, reader: svc.reader(), client: client(), walletKind: null, ...svc.rail() },
    store,
    guard: createSpendGuard()
  })

  /** The newer of the checkout's own progress and the record's copy of it. */
  const latest = (a: GroupProgress | null, b: GroupProgress | null): GroupProgress | null => {
    if (!a || !b) return a ?? b
    if (a.updatedAt !== b.updatedAt) return a.updatedAt > b.updatedAt ? a : b
    return RANK[a.status] >= RANK[b.status] ? a : b
  }

  /**
   * Takes an unsettled operation to its end and never resubmits it. It settles only on authoritative
   * evidence: the checkout's own confirmed or failed state, the executor's watcher (the receipt of the
   * hash, or the server's record of the operation's own credit), or a known hash's receipt on its chain.
   * Anything unknown (an unreadable answer, no receipt yet) stays unsettled and is read again; no timer
   * ends it.
   *
   * Nothing is ever signed or sent before the operation's record holds its checkpoint (`durable`, read
   * back from storage): signing needs the credit kept, the hand-off needs `submitted` kept. So a record
   * with no checkpoint, or one at its reservation, with no sign of a hand-off, provably let nothing leave
   * this browser: its reserved credit is given back and it ends cancelled. Any sign of a hand-off (a
   * pending state, a hash, a submitted checkpoint) is only watched.
   */
  const reconcile = async (op: Operation): Promise<void> => {
    if (op.watching || !op.buyer) return
    op.watching = true
    try {
      const store = (op.store ??= ledger.storeFor(op))
      const deadline = svc.now() + WATCH_FOR_MS
      for (let attempt = 0; ; attempt++) {
        const progress = latest(store.getSnapshot().checkout?.groups[0] ?? null, op.progress ?? null)
        const handedOff = op.state === 'pending' || !!op.txHash || !!progress?.txHash || (progress != null && (progress.status === 'submitted' || progress.status === 'pending'))
        if (progress?.status === 'confirmed') {
          settled(op, 'succeeded', { txHash: progress.txHash ?? op.txHash ?? null })
          return
        }
        if (progress && (progress.status === 'failed' || progress.status === 'expired')) {
          settled(op, 'failed', {}, progress.failure?.message)
          return
        }
        if (!progress) {
          if (!handedOff) {
            settled(op, 'cancelled')
            return
          }
          // A hand-off known only by its hash (the checkpoint is gone): its receipt on its chain decides.
          if (op.txHash && op.chainId) {
            const receipt = await svc
              .reader()
              .receipt(op.chainId as Parameters<ChainReader['receipt']>[0], op.txHash)
              .catch(() => null)
            if (receipt?.status === 'success') {
              settled(op, 'succeeded', { txHash: op.txHash })
              return
            }
            if (receipt?.status === 'reverted') {
              settled(op, 'failed', {}, 'The transaction failed on-chain. Nothing was bought.')
              return
            }
          }
        } else if (!handedOff) {
          // Reserved, or not even that, and provably not handed off: give the credit back.
          if (progress.status === 'authorizing') {
            const state: PurchaseState = store.getSnapshot().checkout ?? { id: op.id, account: op.buyer, groups: [progress], createdAt: op.createdAt }
            await releaseAuthorizing(watchDeps(op, store), { ...state, groups: [progress] })
          }
          settled(op, 'cancelled')
          return
        } else {
          const following: GroupProgress =
            progress.status === 'submitted' || progress.status === 'pending' ? progress : { ...progress, status: op.txHash || progress.txHash ? 'pending' : 'submitted', txHash: progress.txHash ?? op.txHash ?? null }
          const result = await svc.watch(watchDeps(op, store), following).catch((): GroupResult => ({ status: 'pending', txHash: following.txHash }))
          if (result.status === 'confirmed') {
            settled(op, 'succeeded', { txHash: result.txHash })
            return
          }
          if (result.status === 'failed') {
            settled(op, 'failed', {}, result.failure.kind === 'reverted' ? 'The transaction failed on-chain. Nothing was bought and your Credits were returned.' : result.failure.message)
            return
          }
        }
        if (svc.now() >= deadline) return
        await svc.sleep(WATCH_BACKOFF_MS[Math.min(attempt, WATCH_BACKOFF_MS.length - 1)])
      }
    } finally {
      op.watching = false
    }
  }

  /** This buyer's operations from storage, once per page: an open dialog didn't survive, a hand-off did. */
  const restore = (buyer: string): void => {
    if (restoredFor === buyer) return
    restoredFor = buyer
    for (const record of ledger.load(buyer)) {
      if (operations.has(record.id)) continue
      const op: Operation = { ...record }
      operations.set(op.id, op)
      if (op.state === 'reviewing') move(op, 'cancelled')
      else if (op.state === 'processing' || op.state === 'pending') void reconcile(op)
    }
  }

  // ---- Reading the terms and confirming ---------------------------------------------------------

  const create = (req: ShopBuyRequest, buyer: string | null): Operation => {
    const at = svc.now()
    const op: Operation = { id: svc.newId(), buyer, item: req.item, state: 'reviewing', createdAt: at, updatedAt: at }
    operations.set(op.id, op)
    publish(op)
    return op
  }

  /**
   * TRY AGAIN, after an attempt that provably handed nothing off: that attempt ends, and a fresh
   * operation (its own record, checkout and durability) reads the terms anew, so a storage that failed
   * once and works again doesn't keep refusing.
   */
  const restart = (op: Operation, req: ShopBuyRequest, ended: 'cancelled' | 'failed'): void => {
    op.unwatchWallet?.()
    op.unwatchWallet = undefined
    move(op, ended)
    const fresh = create(req, signedIn()?.buyer ?? null)
    dialog = fresh
    void review(fresh, req)
  }

  /** Reads the terms and shows them: Buy Item, or Insufficient Funds when the balance is short. */
  const review = async (op: Operation, req: ShopBuyRequest): Promise<void> => {
    const who = signedIn()
    if (!who) {
      show(op, { step: 'signIn' }, {
        onClose: () => close(op, 'cancelled'),
        onSignIn: () => {
          close(op, 'cancelled')
          deps.signIn()
        }
      })
      return
    }
    const kind = deps.wallet?.kind(who.buyer) ?? null
    if (!kind || !deps.wallet?.provider(who.buyer)) {
      fail(op, "Your sign-in can't confirm purchases here. Sign in again on the website to buy with Credits.")
      return
    }
    show(op, { step: 'loading' }, { onClose: () => close(op, 'cancelled') })
    let quote: Quote | string
    try {
      quote = await readQuote(op, who.login, who.buyer, kind, req)
    } catch (e) {
      console.warn('[shell/shop] could not read the purchase terms', e)
      if (op.state === 'reviewing' && dialog === op) fail(op, "We couldn't check this item's price. Try again in a moment.", () => restart(op, req, 'cancelled'))
      return
    }
    // Cancelled while it loaded.
    if (op.state !== 'reviewing' || dialog !== op) return
    if (typeof quote === 'string') fail(op, quote)
    else present(op, req, quote)
  }

  const readQuote = async (op: Operation, login: StoredLogin, buyer: string, kind: ShopWalletKind, req: ShopBuyRequest): Promise<Quote | string> => {
    const c = client()
    await svc.primeFlags().catch(() => {})
    const [listing, balance] = await Promise.all([svc.fetchItem(req.item.contract, req.item.itemId, c), svc.balance(buyer, c)])
    const input = listing ? cartInput(listing) : null
    if (!listing || !input) return "This item isn't on sale in the Shop right now."
    const account = parseEvmAddress(buyer)
    if (!account) return "Your sign-in can't confirm purchases here."
    // A fresh line each time the terms are read (a retry included): nothing was handed off before them.
    const store = (op.store = ledger.storeFor(op))
    store.replaceCheckout(null)
    store.clear()
    const added = store.add(input)
    store.setMethod(added.id, 'credits')
    const line = store.getSnapshot().lines.find((l) => l.id === added.id) ?? added
    const session: Session = kind === 'web3' ? { kind: 'web3', wallet: account } : { kind: 'web2', account }
    const offers = await svc.offers(c)([{ key: line.id, asset: line.asset, listing: line.listing, quantity: 1 }], session, AbortSignal.timeout(15_000))
    const offer = offers[line.id] ?? null
    if (!offer) return "This item isn't on sale in the Shop right now."
    const partition = partitionCart([{ line, offer }], session, { creditsBalance: balance.balanceCents })
    const group = partition.groups.find((g) => g.key.method === 'credits')
    if (!group || group.total.method !== 'credits') return "This item can't be bought with Credits. You can still buy it on the website."
    const item: ShellPurchaseItem = {
      name: listing.name,
      creator: req.creatorName || shortAddress(listing.creator ?? ''),
      rarity: listing.rarity || 'base',
      thumbnail: listing.thumbnail || undefined,
      category: listing.category === 'emote' ? 'emote' : (listing.wearableCategory ?? listing.category),
      credits: creditsCeil(group.total.charged)
    }
    op.display = item
    return { login: loginKey(login), buyer, kind, session, client: c, line, offer, group, item, balance: balance.credits }
  }

  /** While the terms are up, a wallet that leaves the buyer's account makes them stale. */
  const watchWallet = (op: Operation, quote: Quote): void => {
    const provider = deps.wallet?.provider(quote.buyer)
    if (!provider?.on || !provider.removeListener) return
    const onAccounts = (accounts: unknown): void => {
      const first = Array.isArray(accounts) && typeof accounts[0] === 'string' ? accounts[0].toLowerCase() : ''
      if (first !== quote.buyer) quote.stale = 'Your wallet changed account. Open the item again to buy it.'
    }
    const onGone = (): void => {
      quote.stale = 'Your wallet disconnected. Open the item again to buy it.'
    }
    provider.on('accountsChanged', onAccounts)
    provider.on('disconnect', onGone)
    op.unwatchWallet = () => {
      provider.removeListener?.('accountsChanged', onAccounts)
      provider.removeListener?.('disconnect', onGone)
    }
  }

  const present = (op: Operation, req: ShopBuyRequest, quote: Quote): void => {
    const short = quote.item.credits - quote.balance
    if (short > 0) {
      show(op, { step: 'insufficient', item: quote.item, balance: quote.balance, need: short }, {
        onClose: () => close(op, 'cancelled'),
        onBuyCredits: () => {
          deps.openExternal(svc.creditsUrl())
          close(op, 'cancelled')
        }
      })
      return
    }
    show(op, { step: 'confirm', item: quote.item, balance: quote.balance }, { onClose: () => close(op, 'cancelled'), onConfirm: () => void settle(op, req, quote) })
    watchWallet(op, quote)
  }

  // ---- Paying -----------------------------------------------------------------------------------

  /** BUY ITEM: the confirmed terms, for the same login and wallet account, through the web Shop's executor. */
  const settle = async (op: Operation, req: ShopBuyRequest, quote: Quote): Promise<void> => {
    if (op.state !== 'reviewing' || dialog !== op) return
    op.unwatchWallet?.()
    // Every TRY AGAIN offered below follows an attempt that handed nothing off.
    const again = (): void => restart(op, req, 'cancelled')
    const current = (): boolean => {
      const who = signedIn()
      return !!who && who.buyer === quote.buyer && loginKey(who.login) === quote.login && deps.wallet?.kind(quote.buyer) === quote.kind && !quote.stale
    }
    const wallet = deps.wallet?.provider(quote.buyer)
    if (!wallet || !current()) {
      fail(op, quote.stale ?? 'Your sign-in changed meanwhile. Open the item again to buy it.')
      return
    }
    const chainId = quote.group.key.chainId
    const manager = svc.manager(chainId)
    const call = quote.group.key.call.kind
    if (!manager || call === 'one') {
      fail(op, "Buying with Credits isn't available here right now.")
      return
    }
    // The wallet's selected account, before anything is reserved for the purchase.
    try {
      await assertWalletAccount(wallet, quote.buyer, quote.kind === 'web3')
    } catch (e) {
      fail(op, e instanceof WalletRefusal ? e.message : "Your wallet didn't answer. Try again.", again)
      return
    }
    if (op.state !== 'reviewing' || dialog !== op) return
    // The login may have changed while the wallet answered.
    if (!current()) {
      fail(op, quote.stale ?? 'Your sign-in changed meanwhile. Open the item again to buy it.')
      return
    }
    // A purchase that can't be kept can't be followed after a reload: nothing is reserved without it.
    if (!ledger.writable()) {
      fail(op, "This browser can't keep a record of the purchase right now (its storage is full or blocked), so nothing was bought.", again)
      return
    }
    const intent: PurchaseIntent = {
      buyer: quote.buyer,
      chainId,
      manager,
      item: req.item,
      sale: { venue: quote.group.key.contract.toLowerCase(), call },
      current,
      durable: (stage) => durable(op, stage)
    }
    op.chainId = chainId
    move(op, 'processing')
    if (ledger.failed(op.id) || ledger.saved(op.buyer ?? '', op.id)?.state !== 'processing') {
      op.state = 'reviewing'
      publish(op)
      fail(op, "This browser can't keep a record of the purchase right now (its storage is full or blocked), so nothing was bought.", again)
      return
    }
    show(op, { step: 'processing', balance: quote.balance }, { onClose: () => {} })
    const store = (op.store ??= ledger.storeFor(op))
    // The relay gets the signed purchase only once its hand-off is kept; refused here, it was never sent.
    const rail = svc.rail()
    const send = rail.fetchImpl ?? ((...a: Parameters<typeof fetch>) => fetch(...a))
    const relay = (async (input: Parameters<typeof fetch>[0], init?: RequestInit) => {
      if ((init?.method ?? 'GET').toUpperCase() === 'POST' && !durable(op, 'handedOff')) {
        return new Response(JSON.stringify({ ok: false, message: "This browser couldn't keep a record of the purchase, so it wasn't sent." }), { status: 409, headers: { 'content-type': 'application/json' } })
      }
      return send(input, init)
    }) as typeof fetch
    let unmirror = (): void => {}
    let result: GroupResult
    try {
      const review = await reviewCart([{ line: quote.line, offer: quote.offer }], quote.session, quote.buyer, svc.resolvers(quote.client))
      const resolved = new Map(review.buyable.map((r) => [r.line.id, r]))
      if (!resolved.has(quote.line.id)) {
        const why = review.own.length ? 'You already own this item.' : review.stale.length ? 'Its price changed. Try again to see the new one.' : "This item can't be bought right now."
        op.state = 'reviewing'
        publish(op)
        fail(op, why, review.stale.length ? again : undefined)
        return
      }
      store.startCheckout([quote.group])
      // Every step the executor writes to the checkout is copied into the operation's record.
      const mirror = (): void => {
        const group = store.getSnapshot().checkout?.groups[0]
        if (!group) return
        op.progress = { ...group }
        keep(op)
      }
      mirror()
      unmirror = store.subscribe(mirror)
      result = await svc.run(
        {
          ctx: { buyer: quote.buyer, provider: svc.guard(wallet, intent), reader: svc.reader(), client: quote.client, walletKind: quote.kind, ...rail, fetchImpl: relay },
          store,
          guard: createSpendGuard()
        },
        quote.group,
        resolved
      )
      unmirror()
    } catch (e) {
      unmirror()
      console.warn('[shell/shop] the purchase stopped unexpectedly', e)
      // The executor answers every hand-off itself, so a throw should come before any; if the checkout
      // got as far as a credit, it is followed like any purchase that may have gone out, never retried.
      if (op.progress && op.progress.creditIds.length > 0) {
        move(op, 'pending')
        showPending(op)
        void reconcile(op)
        return
      }
      op.state = 'reviewing'
      publish(op)
      fail(op, "We couldn't start this purchase. Nothing was charged.", again)
      return
    }
    switch (result.status) {
      case 'confirmed':
        move(op, 'succeeded', { txHash: result.txHash })
        showSuccess(op)
        return
      case 'pending':
        move(op, 'pending', { txHash: result.txHash })
        showPending(op)
        void reconcile(op)
        return
      case 'failed': {
        op.state = 'reviewing'
        publish(op)
        // The balance fell short between the quote and the authorization: Insufficient Funds after all.
        if (result.topUp) {
          present(op, req, { ...quote, balance: creditsFloor(result.topUp.balanceCents as UsdCents) })
          return
        }
        // A failed group handed nothing off, or its transaction reverted: either way this attempt is over.
        fail(op, result.failure.message, result.failure.kind !== 'refused' ? () => restart(op, req, 'failed') : undefined)
        return
      }
      case 'skipped':
        op.state = 'reviewing'
        publish(op)
        fail(op, "This purchase couldn't run. Nothing was charged.", again)
    }
  }

  // ---- Requests ---------------------------------------------------------------------------------

  const buy = (params: unknown): ShopOperation => {
    const req = parseBuyRequest(params)
    if (dialog && (dialog.state === 'reviewing' || dialog.state === 'processing')) throw new Error('a purchase is already open')
    const who = signedIn()
    if (who) {
      restore(who.buyer)
      // The same item while a purchase of it is unsettled: that purchase, never a second one.
      const unsettled = [...operations.values()].find(
        (o) => o.buyer === who.buyer && o.item.contract === req.item.contract && o.item.itemId === req.item.itemId && (o.state === 'processing' || o.state === 'pending')
      )
      if (unsettled) {
        unsettled.input = req.input
        showPending(unsettled)
        void reconcile(unsettled)
        return snapshot(unsettled)
      }
    }
    const op = create(req, who?.buyer ?? null)
    op.input = req.input
    void review(op, req)
    return snapshot(op)
  }

  const capabilities = (): ShopCapabilities => {
    const login = deps.login()
    if (!login || loginExpired(login)) return { v: SHOP_PROTOCOL, buyer: null, wallet: null, credits: false, reason: 'signedOut' }
    const buyer = rootAddress(login.identity).toLowerCase()
    if (login.guest) return { v: SHOP_PROTOCOL, buyer, wallet: null, credits: false, reason: 'guest' }
    const wallet = deps.wallet?.kind(buyer) ?? null
    const usable = wallet != null && deps.wallet?.provider(buyer) != null && !!login.identity.ephemeralIdentity.privateKey
    return { v: SHOP_PROTOCOL, buyer, wallet, credits: usable, ...(usable ? {} : { reason: 'noWallet' as const }) }
  }

  const balance = async (): Promise<ShopBalance | null> => {
    const who = signedIn()
    if (!who) return null
    const b = await svc.balance(who.buyer, client())
    return { v: SHOP_PROTOCOL, credits: b.credits }
  }

  /** One operation by id; with no id, the signed-in buyer's, newest first. */
  const status = (params: unknown): ShopOperation | ShopOperation[] | null => {
    const who = signedIn()
    if (who) restore(who.buyer)
    const empty = params == null || (typeof params === 'object' && Object.keys(params as object).length === 0)
    if (empty) {
      if (!who) return []
      // Newest first; the later of two made in the same millisecond first too (the sort is stable).
      return [...operations.values()].filter((o) => o.buyer === who.buyer).reverse().sort((a, b) => b.createdAt - a.createdAt).map(snapshot)
    }
    const op = operations.get(parseOperationRef(params))
    return op ? snapshot(op) : null
  }

  return {
    async request(method, params) {
      switch (method) {
        case SHOP_METHODS.capabilities: {
          const who = signedIn()
          if (who) restore(who.buyer)
          return capabilities()
        }
        case SHOP_METHODS.balance:
          return balance()
        case SHOP_METHODS.buy:
          return buy(params)
        case SHOP_METHODS.status:
          return status(params)
        default:
          throw new Error(`unknown request ${method}`)
      }
    }
  }
}
