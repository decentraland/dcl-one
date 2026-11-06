// The web shell's side of the in-world Shop purchase (lib/shopProtocol). The HUD asks to buy an item; this
// reads the item's listing, the server's offer and the buyer's Credits balance itself, shows the buyer the
// design's dialog over the app (./overlay), and on BUY ITEM settles the purchase through the web Shop's own
// checkout (cart partition, review, the per-group executor and its rails), with the wallet the host
// injected. The buyer is the root signer of the login this shell holds; the HUD never names it.
//
// One operation at a time. Its state goes to the HUD as `bevy-shell:shop` events and answers shop.status,
// so a HUD that reloads mid-purchase can follow it. A purchase handed off to the wallet or the relay is
// never failed or retried by a timeout: it stays pending, as the executor leaves it.

import type { AuthIdentity as SignerIdentity } from '../../../data/auth/identity'
import type { Eip1193Provider } from '../../../data/auth/wallet'
import { createShopClient, type ShopClient } from '../../../shop/api/shop'
import { purchaseOfferSource } from '../../../shop/api/offers'
import { identitySigner, type ShopSigner } from '../../../shop/api/signer'
import { partitionCart, type CartGroup } from '../../../shop/cart/partition'
import { createCartStore, type CartAddInput } from '../../../shop/cart/store'
import { shopChainReader, type ChainReader } from '../../../shop/checkout/chain/rpc'
import { liveResolvers, reviewCart, type SettlementResolvers } from '../../../shop/checkout/purchase/cart'
import { runGroup, type CheckoutDeps, type GroupResult } from '../../../shop/checkout/purchase/executor'
import { createSpendGuard } from '../../../shop/checkout/purchase/spend-guard'
import { getUsdBalance, type UsdBalance } from '../../../shop/credits/credits'
import type { OfferSource } from '../../../shop/detail/offers'
import { fetchShopListingForItem } from '../../../shop/detail/api'
import type { CatalogItem } from '../../../shop/discovery/lib/catalogItem'
import { fetchRemoteFlags, primeRemoteFlags } from '../../../shop/discovery/lib/remoteFlags'
import { parseEvmAddress } from '../../../shop/model/address'
import { itemAsset } from '../../../shop/model/asset'
import { creditsCeil, creditsFloor, type UsdCents } from '../../../shop/model/money'
import type { CartLine, PurchaseOffer } from '../../../shop/model/purchase'
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
import { guardedProvider } from './walletGuard'

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
  show(view: ShellPurchaseView, handlers: ShellPurchaseHandlers): void
  hide(): void
}

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
  run(deps: CheckoutDeps, group: CartGroup, resolved: Parameters<typeof runGroup>[2]): Promise<GroupResult>
  /** The wallet as the purchase may use it (./walletGuard). */
  guard(provider: Eip1193Provider, buyer: string): Eip1193Provider
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

const LIVE: ShellShopServices = {
  client: (signer) => createShopClient({ signer }),
  fetchItem: (contract, itemId, client) => fetchShopListingForItem(contract, itemId, { client }),
  offers: (client) => purchaseOfferSource(client),
  balance: (account, client) => getUsdBalance(account, client),
  resolvers: (client) => liveResolvers(client),
  reader: () => shopChainReader(),
  primeFlags: async () => primeRemoteFlags(await fetchRemoteFlags(AbortSignal.timeout(FLAGS_TIMEOUT_MS))),
  run: runGroup,
  guard: (provider, buyer) => guardedProvider(provider, buyer),
  newId: () => crypto.randomUUID(),
  creditsUrl: () => withUtm(siteUrl('/shop/credits'))
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
  login: StoredLogin
  buyer: string
  kind: ShopWalletKind
  session: Session
  client: ShopClient
  line: CartLine
  offer: PurchaseOffer
  group: CartGroup
  item: ShellPurchaseItem
  balance: number
}

interface Operation {
  id: string
  item: { contract: string; itemId: string }
  state: ShopOperationState
  next?: 'backpack'
  txHash?: string | null
}

export interface ShellShop {
  /** Answers a `shop.*` request from the app frame. */
  request(method: string, params: unknown): Promise<unknown>
}

export function createShellShop(deps: ShellShopDeps): ShellShop {
  const svc: ShellShopServices = { ...LIVE, ...deps.services }
  let current: Operation | null = null

  const snapshot = (op: Operation): ShopOperation => ({
    v: SHOP_PROTOCOL,
    operation: op.id,
    state: op.state,
    item: op.item,
    ...(op.next ? { next: op.next } : {}),
    ...(op.txHash !== undefined ? { txHash: op.txHash } : {})
  })
  const publish = (op: Operation): void => deps.post({ type: SHOP_EVENT, ...snapshot(op) })
  const move = (op: Operation, state: ShopOperationState, extra: Partial<Operation> = {}): void => {
    if (TERMINAL_STATES.includes(op.state)) return
    Object.assign(op, extra, { state })
    publish(op)
  }
  const close = (op: Operation, state: ShopOperationState): void => {
    deps.ui.hide()
    move(op, state)
  }

  const signedIn = (): { login: StoredLogin; buyer: string } | null => {
    const login = deps.login()
    if (!login || login.guest || loginExpired(login) || !login.identity.ephemeralIdentity.privateKey) return null
    return { login, buyer: rootAddress(login.identity).toLowerCase() }
  }

  const fail = (op: Operation, message: string, retry?: () => void): void => {
    deps.ui.show(
      { step: 'failed', message, retry: retry != null },
      {
        onClose: () => close(op, 'failed'),
        ...(retry ? { onRetry: retry } : {})
      }
    )
  }

  /** Reads the terms and shows them: Buy Item, or Insufficient Funds when the balance is short. */
  const review = async (op: Operation, req: ShopBuyRequest): Promise<void> => {
    const who = signedIn()
    if (!who) {
      deps.ui.show(
        { step: 'signIn' },
        {
          onClose: () => close(op, 'cancelled'),
          onSignIn: () => {
            close(op, 'cancelled')
            deps.signIn()
          }
        }
      )
      return
    }
    const kind = deps.wallet?.kind(who.buyer) ?? null
    if (!kind || !deps.wallet?.provider(who.buyer)) {
      fail(op, "Your sign-in can't confirm purchases here. Sign in again on the website to buy with Credits.")
      return
    }
    deps.ui.show({ step: 'loading' }, { onClose: () => close(op, 'cancelled') })
    let quote: Quote
    try {
      const q = await readQuote(who.login, who.buyer, kind, req)
      if (typeof q === 'string') {
        if (op.state === 'reviewing') fail(op, q)
        return
      }
      quote = q
    } catch (e) {
      console.warn('[shell/shop] could not read the purchase terms', e)
      if (op.state === 'reviewing') fail(op, "We couldn't check this item's price. Try again in a moment.", () => void review(op, req))
      return
    }
    // Cancelled while it loaded.
    if (op.state !== 'reviewing') return
    present(op, req, quote)
  }

  const readQuote = async (login: StoredLogin, buyer: string, kind: ShopWalletKind, req: ShopBuyRequest): Promise<Quote | string> => {
    const client = svc.client(() => loginSigner(deps.login()))
    await svc.primeFlags().catch(() => {})
    const [listing, balance] = await Promise.all([svc.fetchItem(req.item.contract, req.item.itemId, client), svc.balance(buyer, client)])
    const input = listing ? cartInput(listing) : null
    if (!listing || !input) return "This item isn't on sale in the Shop right now."
    const store = createCartStore({ storage: () => null })
    store.scopeTo(buyer)
    const added = store.add(input)
    store.setMethod(added.id, 'credits')
    const line = store.getSnapshot().lines.find((l) => l.id === added.id) ?? added
    const account = parseEvmAddress(buyer)
    if (!account) return "Your sign-in can't confirm purchases here."
    const session: Session = kind === 'web3' ? { kind: 'web3', wallet: account } : { kind: 'web2', account }
    const offers = await svc.offers(client)([{ key: line.id, asset: line.asset, listing: line.listing, quantity: 1 }], session, AbortSignal.timeout(15_000))
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
    return { login, buyer, kind, session, client, line, offer, group, item, balance: balance.credits }
  }

  const present = (op: Operation, req: ShopBuyRequest, quote: Quote): void => {
    const short = quote.item.credits - quote.balance
    if (short > 0) {
      deps.ui.show(
        { step: 'insufficient', item: quote.item, balance: quote.balance, need: short },
        {
          onClose: () => close(op, 'cancelled'),
          onBuyCredits: () => {
            deps.openExternal(svc.creditsUrl())
            close(op, 'cancelled')
          }
        }
      )
      return
    }
    deps.ui.show(
      { step: 'confirm', item: quote.item, balance: quote.balance },
      {
        onClose: () => close(op, 'cancelled'),
        onConfirm: () => void settle(op, req, quote)
      }
    )
  }

  /** BUY ITEM: the confirmed terms, for the same login and wallet, through the web Shop's executor. */
  const settle = async (op: Operation, req: ShopBuyRequest, quote: Quote): Promise<void> => {
    if (op.state !== 'reviewing') return
    const who = signedIn()
    const wallet = who && who.buyer === quote.buyer ? deps.wallet?.provider(quote.buyer) : null
    if (!who || !wallet || deps.wallet?.kind(quote.buyer) !== quote.kind) {
      fail(op, 'You signed in with another account meanwhile. Open the item again to buy it with this one.')
      return
    }
    move(op, 'processing')
    deps.ui.show({ step: 'processing', balance: quote.balance }, { onClose: () => {} })
    let result: GroupResult
    try {
      const review = await reviewCart([{ line: quote.line, offer: quote.offer }], quote.session, quote.buyer, svc.resolvers(quote.client))
      const resolved = new Map(review.buyable.map((r) => [r.line.id, r]))
      if (!resolved.has(quote.line.id)) {
        const why = review.own.length
          ? 'You already own this item.'
          : review.stale.length
            ? 'Its price changed. Try again to see the new one.'
            : "This item can't be bought right now."
        op.state = 'reviewing'
        publish(op)
        fail(op, why, review.stale.length ? () => void retry(op, req) : undefined)
        return
      }
      const store = createCartStore({ storage: () => null })
      store.scopeTo(quote.buyer)
      store.restore([quote.line])
      store.startCheckout([quote.group])
      result = await svc.run(
        { ctx: { buyer: quote.buyer, provider: svc.guard(wallet, quote.buyer), reader: svc.reader(), client: quote.client, walletKind: quote.kind }, store, guard: createSpendGuard() },
        quote.group,
        resolved
      )
    } catch (e) {
      // The executor answers every hand-off itself; a throw here came before any.
      console.warn('[shell/shop] the purchase could not start', e)
      op.state = 'reviewing'
      publish(op)
      fail(op, "We couldn't start this purchase. Nothing was charged.", () => void retry(op, req))
      return
    }
    switch (result.status) {
      case 'confirmed':
        move(op, 'succeeded', { txHash: result.txHash })
        deps.ui.show(
          { step: 'success', item: quote.item },
          {
            onClose: () => deps.ui.hide(),
            onBackpack: () => {
              deps.ui.hide()
              op.next = 'backpack'
              publish(op)
            }
          }
        )
        return
      case 'pending':
        move(op, 'pending', { txHash: result.txHash })
        deps.ui.show({ step: 'pending' }, { onClose: () => deps.ui.hide() })
        return
      case 'failed': {
        op.state = 'reviewing'
        publish(op)
        // The balance fell short between the quote and the authorization: Insufficient Funds after all.
        if (result.topUp) {
          present(op, req, { ...quote, balance: creditsFloor(result.topUp.balanceCents as UsdCents) })
          return
        }
        fail(op, result.failure.message, result.failure.kind !== 'refused' ? () => void retry(op, req) : undefined)
        return
      }
      case 'skipped':
        op.state = 'reviewing'
        publish(op)
        fail(op, "This purchase couldn't run. Nothing was charged.", () => void retry(op, req))
    }
  }
  // Trying again reads the terms again: a new quote and a new confirmation, never a resubmission.
  const retry = (op: Operation, req: ShopBuyRequest): Promise<void> => review(op, req)

  const buy = (params: unknown): ShopOperation => {
    const req = parseBuyRequest(params)
    // One dialog at a time. A purchase left pending is the chain's and the server's to settle now: it
    // doesn't hold the next one back.
    if (current && (current.state === 'reviewing' || current.state === 'processing')) throw new Error('a purchase is already open')
    const op: Operation = { id: svc.newId(), item: req.item, state: 'reviewing' }
    current = op
    publish(op)
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
    const b = await svc.balance(who.buyer, svc.client(() => loginSigner(deps.login())))
    return { v: SHOP_PROTOCOL, credits: b.credits }
  }

  return {
    async request(method, params) {
      switch (method) {
        case SHOP_METHODS.capabilities:
          return capabilities()
        case SHOP_METHODS.balance:
          return balance()
        case SHOP_METHODS.buy:
          return buy(params)
        case SHOP_METHODS.status: {
          const id = parseOperationRef(params)
          return current && current.id === id ? snapshot(current) : null
        }
        default:
          throw new Error(`unknown request ${method}`)
      }
    }
  }
}
