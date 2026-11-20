// The web shell's side of the in-world Shop purchase (library/ui/apps/shop/shopProtocol). The HUD asks to buy items (one,
// or its cart's); this reads their listings, the server's offers and the buyer's Credits balance itself,
// shows the buyer the design's dialog over the app (./overlay), and on BUY ITEM settles the purchase
// through the web Shop's own checkout (cart partition, review, the per-group executor and its rails), with
// the wallet the host injected, bound to the confirmed purchase (./walletGuard). The buyer is the root
// signer of the login this shell holds; the HUD never names it.
//
// The checkout partitions the items into groups, each one transaction paid with one credit (the store's
// mints together, a marketplace's trades together). Each group is an operation of its own, run one after
// another under the one confirmation: kept per buyer (./ledger) with its own checkout, its state going to
// the HUD as `bevy-shell:shop` events and answering shop.status. A purchase handed off to the wallet or the
// relay is never failed or retried by a timeout: it stays pending and is watched (the executor's
// watchGroup: its receipt, or the server's record of its credit) until it settles, across dialogs and
// reloads. Buying an item again while one of it is unresolved shows that one instead.

import { fetchProfile, profileFaceUrl } from '../../../data/catalyst/profile'
import type { Eip1193Provider } from '../../../data/auth/wallet'
import { createShopClient, type ShopClient } from '@shop/api/shop'
import { purchaseOfferSource } from '../../../shop/api/offers'
import { type ShopSigner } from '@shop/api/signer'
import { partitionCart, type CartGroup } from '@shop/cart/partition'
import type { CartAddInput, CartStore } from '../../../shop/cart/store'
import { shopChainReader, type ChainReader } from '../../../shop/checkout/chain/rpc'
import { liveResolvers, reviewCart, type SettlementResolvers } from '../../../shop/checkout/purchase/cart'
import { releaseAuthorizing, runGroup, watchGroup, type CheckoutDeps, type GroupResult } from '../../../shop/checkout/purchase/executor'
import type { RailContext } from '../../../shop/checkout/purchase/rail'
import { createSpendGuard } from '@shop/checkout/purchase/spend-guard'
import { fetchUserPurchases, getUsdBalance, type UsdBalance } from '@shop/credits/credits'
import type { OfferSource } from '../../../shop/detail/offers'
import { fetchShopItems } from '@shop/discovery/api'
import { isResale } from '@shop/market/access-rule'
import { ethereumChainId, settlementChainId } from '../../../shop/checkout/chain/config'
import { rpcReader } from '../../../shop/land/evm'
import { readBannedNames } from '../../../shop/names/reads'
import { checkNameAvailability } from '@shop/names/name'
import { quoteNameCredits } from '@shop/names/api'
import type { CatalogItem } from '@shop/discovery/lib/catalogItem'
import { primeRemoteFlags, readRemoteFlags, remoteFlag } from '../../../shop/discovery/lib/remoteFlags'
import { parseEvmAddress } from '@shop/model/address'
import { chainLabel } from '@shop/model/chain'
import { isRejection } from '../../../shop/checkout/lib/errors'
import { readManaBalanceWei } from '../../../shop/checkout/lib/mana'
import { REMOTE_FLAG } from '@shop/discovery/lib/rolloutFlags'
import { requestChainSwitch } from '../../../shop/wallet/chain'
import { itemAsset } from '@shop/model/asset'
import { creditsCeil, creditsFloor, type UsdCents } from '@shop/model/money'
import type { CartLine, GroupProgress, GroupStatus, PurchaseOffer, PurchaseState } from '@shop/model/purchase'
import type { Session } from '@shop/model/settlement'
import { loginExpired, rootAddress, type StoredLogin } from '@explorer-core/features/auth/sso'
import type { ShopPaymentMethod, ShopPurchaseItem, ShopPurchaseStep } from '@dcl/one-ui/apps/shop/ui'
import { manaGuardedProvider, type ManaPurchaseIntent, type ManaSaleTerms } from './manaGuard'
import { SEND_MS } from './manaPurchase'
import { boundedReader, createManaEvidence, EVIDENCE_READ_MS, type ManaEvidence } from './manaEvidence'
import { checkoutSiteUrl } from '../../lib/baseDomain'
import {
  parseBuyRequest,
  parseOperationRef,
  shopBusy,
  SHOP_EVENT,
  SHOP_METHODS,
  SHOP_PROTOCOL,
  isTerminal,
  MAX_PURCHASE_UNITS,
  sameItem,
  unitsOf,
  TERMINAL_STATES,
  type ShopBalance,
  type ShopBuyRequest,
  type ShopCapabilities,
  type ShopOperation,
  type ShopOperationEvent,
  type ShopItemOperation,
  type ShopLine,
  type ShopNamesMode,
  type ShopOperationState,
  type ShopWalletKind
} from '@dcl/one-ui/apps/shop/shopProtocol'
import { withUtm } from '@explorer-core/lib/utm'
import { trustedCreditsManager, type TrustedManager } from './config'
import { createLedger, STORAGE_REFUSED, type OperationRecord } from './ledger'
import { createManaFlow, MANA_BALANCE_TIMEOUT_MS } from './manaFlow'
import { seesMana } from './manaGate'
import { createNamePurchase, nameBuild, namesModeFor, type NameStanding } from './namePurchase'
import { assertWalletAccount, guardedProvider, WalletRefusal, type PurchaseIntent } from './walletGuard'
import { hostApiOptions } from '../../../shop/api/host'
import { loginSigner } from '../loginSigner'
import { webLock } from '../webLock'
import { hostOr, shopFlags } from '../../../shop/discovery/flags'

/** The buyer's wallet, as the page hosting the shell resolves it (Sites: its sign-in session). */
export interface ShellShopWallet {
  /** How `account` signs here: an in-app (managed) wallet, a browser (web3) wallet, or neither. */
  kind(account: string): ShopWalletKind | null
  /** That wallet as an EIP-1193 provider for `account`, or null when it can't sign for it. */
  provider(account: string): Eip1193Provider | null
  /** Whether `account` may use the Marketplace: a web3 wallet recorded as its sign-in, here to sign. Absent: never. */
  marketplace?(account: string): boolean
}

/** The dialog's item without its glyph: the overlay draws the category's. */
export type ShellPurchaseItem = Omit<ShopPurchaseItem, 'categoryIcon'> & { category?: string }
type WithItems<S> = S extends { items: ShopPurchaseItem[] } ? Omit<S, 'items'> & { items: ShellPurchaseItem[] } : S
export type ShellPurchaseView = WithItems<ShopPurchaseStep>

export interface ShellPurchaseHandlers {
  onClose: () => void
  onConfirm?: () => void
  onBuyCredits?: () => void
  onBackpack?: () => void
  /** A simulated NAME's success: see it over the avatar. */
  onPreview?: () => void
  onRetry?: () => void
  /** SWITCH TO … AND RETRY: move the wallet to the failed step's network, then try again. */
  onSwitchNetwork?: () => void
  onSignIn?: () => void
  /** BUY on Choose your payment method, with the row picked. */
  onPay?: (method: ShopPaymentMethod) => void
  /** AUTHORIZE (or TRY AGAIN) on Authorize MANA. */
  onAuthorize?: () => void
  /** CHECK AGAIN on a purchase still being confirmed. */
  onCheckAgain?: () => void
}

/** Draws the dialog over the app, or takes it away. */
export interface ShellShopUi {
  /** `opening.pointer`: the dialog opens from a pointer press in the app, so its first focus isn't rung. */
  show(view: ShellPurchaseView, handlers: ShellPurchaseHandlers, opening?: { pointer: boolean }): void
  hide(): void
}

/** An operation the dialog can be up for: an item purchase's or a NAME's. */
export interface DialogOwner {
  state: ShopOperationState
}

/**
 * The one dialog over the app, which the item purchases and BUY NAME share. It is up for one operation at
 * a time: whoever shows a step owns it, only its owner takes it away, and an operation that lost it (its
 * dialog cancelled, or replaced by another's) shows nothing more, however late its answers come.
 */
export interface DialogSlot {
  show(owner: DialogOwner, view: ShellPurchaseView, handlers: ShellPurchaseHandlers, opening?: { pointer: boolean }): void
  /** Points the dialog at `owner` without drawing: its next step draws it (a fresh attempt taking over). */
  hand(owner: DialogOwner): void
  hide(owner: DialogOwner): void
  holds(owner: DialogOwner): boolean
  holder(): DialogOwner | null
  /** What it shows now, for whom: the step last drawn, while its owner still holds it. */
  shown(): { owner: DialogOwner; view: ShellPurchaseView; handlers: ShellPurchaseHandlers; opening?: { pointer: boolean } } | null
  /** A purchase is open in it: its operation is still being reviewed or processed. */
  open(): boolean
}

export function createDialogSlot(ui: ShellShopUi): DialogSlot {
  let holder: DialogOwner | null = null
  let drawn: ReturnType<DialogSlot['shown']> = null
  return {
    show(owner, view, handlers, opening) {
      holder = owner
      drawn = { owner, view, handlers, ...(opening ? { opening } : {}) }
      ui.show(view, handlers, opening)
    },
    hand(owner) {
      holder = owner
    },
    hide(owner) {
      if (holder !== owner) return
      holder = null
      drawn = null
      ui.hide()
    },
    holds: (owner) => holder === owner,
    holder: () => holder,
    shown: () => (drawn && drawn.owner === holder ? drawn : null),
    open: () => holder != null && (holder.state === 'reviewing' || holder.state === 'processing')
  }
}

/** What a wallet asks its owner to approve: a signature, a transaction, a network to move to. */
const WALLET_PROMPTS = new Set(['eth_signTypedData_v4', 'eth_signTypedData_v3', 'eth_signTypedData', 'eth_sendTransaction', 'personal_sign', 'eth_sign', 'wallet_switchEthereumChain', 'wallet_addEthereumChain'])

/**
 * `wallet` as `owner`'s purchase uses it: while a self-custody wallet asks the buyer to sign or send, the dialog
 * says to confirm in the wallet, then shows the step it showed before, unless a newer step replaced the prompt
 * meanwhile. The request, its answer and its refusal pass through untouched. An in-app wallet signs without asking,
 * so it is returned as it is; so is a request while `owner` isn't the one shown.
 */
export function promptingWallet(slot: DialogSlot, owner: DialogOwner, wallet: Eip1193Provider, kind: ShopWalletKind): Eip1193Provider {
  if (kind !== 'web3') return wallet
  return {
    request: async (args) => {
      const before = WALLET_PROMPTS.has(args.method) ? slot.shown() : null
      if (!before || before.owner !== owner) return wallet.request(args)
      const asking: ShellPurchaseView = { step: 'wallet' }
      slot.show(owner, asking, { onClose: () => {} }, before.opening)
      try {
        return await wallet.request(args)
      } finally {
        if (slot.shown()?.view === asking) slot.show(owner, before.view, before.handlers, before.opening)
      }
    },
    ...(wallet.on ? { on: wallet.on.bind(wallet) } : {}),
    ...(wallet.removeListener ? { removeListener: wallet.removeListener.bind(wallet) } : {})
  }
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
  /** The creator-sales rollout, which the listing reads from its module (see library/ui/apps/shop/flagsQuery). */
  primeFlags(): Promise<void>
  run(deps: CheckoutDeps, group: CartGroup, resolved: Resolved): Promise<GroupResult>
  watch(deps: CheckoutDeps, progress: GroupProgress): Promise<GroupResult>
  /** The server's record of the operation's own credits: settled, expired unused, or still open. */
  creditRecord(buyer: string, creditIds: string[], client: ShopClient): Promise<'settled' | 'expired' | 'open'>
  /** The creator's display name, read here (its profile), never taken from the app. */
  creatorName(address: string): Promise<string | null>
  /** The creator's profile picture, read here (its profile), or null while it has none. */
  creatorFace(address: string): Promise<string | null>
  /**
   * Takes the operation's lock for this page (Web Locks), held until the returned release or until the
   * page goes; null when another live page holds it (or, with `wait`, once it lets go). A page touches
   * only operations it holds.
   */
  claim(id: string, wait?: boolean): Promise<(() => void) | null>
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
  /** A NAME's price (the credits quote of its 100 MANA) in US cents, or null while none can be given. */
  namePrice(client: ShopClient): Promise<UsdCents | null>
  /** Whether a NAME is free to register, taken, or on the banned list. */
  nameStanding(name: string): Promise<NameStanding>
  /** How this build buys NAMEs on the chains the page settles on now, or null where it doesn't (./namePurchase). */
  namesMode(): ShopNamesMode | null
  /** Whether the purchase dialog may offer paying in MANA (`shop-mana-in-world`: the host's word, else the flags file). */
  manaInWorld(): boolean
  /** The buyer's MANA on a chain, in wei. */
  manaBalance(buyer: string, chainId: number): Promise<bigint>
  /** The wallet bound to one confirmed purchase paid in MANA (./manaGuard). */
  manaGuard(provider: Eip1193Provider, intent: ManaPurchaseIntent): Eip1193Provider
  /** What the chain says about a MANA sale (./manaEvidence). */
  manaEvidence(): ManaEvidence
  /** How long a MANA sale's relay request (`sendMs`) and each of its chain reads (`readMs`) may take before they give way. */
  manaTimeouts(): { sendMs: number; readMs: number }
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
  /** Whether another dialog (Donate's) is up over the app: a purchase waits its turn. */
  otherDialogOpen?(): boolean
  services?: Partial<ShellShopServices>
}

const FLAGS_TIMEOUT_MS = 3000
/** How long BUY NAME waits on each of its reads (the price, the banned list, the registrar). */
const NAME_READ_TIMEOUT_MS = 10_000
/** How long a page reads an unsettled purchase often (backing off); then every WATCH_SLOW_MS. */
const WATCH_FOR_MS = 30 * 60_000
/** How often a purchase still unsettled after WATCH_FOR_MS is read again. */
const WATCH_SLOW_MS = 5 * 60_000
const WATCH_BACKOFF_MS = [5_000, 10_000, 20_000, 40_000, 60_000]

/**
 * The operation's Web Lock, held until the returned release, or until the page goes (the browser lets a
 * closed or crashed page's locks go). Taken only if free, null when another page holds it; with `wait`,
 * queued until that page lets go. Without Web Locks nothing proves this page is the only one running the
 * purchase, so the answer is null: no purchase runs.
 */
export function claimLock(locks: LockManager | undefined, id: string, wait = false): Promise<(() => void) | null> {
  return webLock(locks, `dcl-inworld-shop:operation:${id}`, wait)
}
/** A credit the server reports expired and unused, with no receipt for the hash, is read again this much later
 *  before the purchase is taken as never gone through: past the index's lag, an expired credit can't be spent. */
const EXPIRY_CONFIRM_MS = 15 * 60_000
/** How far each checkout state is along, to tell the newer of two copies when their clocks tie. */
const RANK: Record<GroupStatus, number> = { idle: 0, authorizing: 1, submitted: 2, pending: 3, confirmed: 4, failed: 4, expired: 4 }

function defaultStorage(): Storage | null {
  try {
    return typeof localStorage !== 'undefined' ? localStorage : null
  } catch {
    return null
  }
}

/**
 * The listing BUY settles an item through, read as the in-world grid reads its card: the unified feed grouped by item,
 * whose one row is the server's pick (a native trade first, then the lowest price, a trade before a store mint). So BUY
 * settles the listing the card shows, a creator's MANA-priced (legacy) trade included, which the listings feed alone
 * lacks. Never a resale. A failed read (the feed needs a current MANA/USD rate) rejects: it is not "not on sale".
 */
export async function fetchBuyListing(contract: string, itemId: string, client: ShopClient, creatorSales: () => boolean): Promise<CatalogItem | null> {
  const { items } = await fetchShopItems({ contractAddress: contract, itemId, listingType: 'primary', first: 1 }, { client, creatorSales })
  const row = items[0]
  return row && !isResale(row) ? row : null
}

/** Creators' profiles, read once per page for both their name and their picture; a read that failed is tried again. */
const creatorProfiles = new Map<string, ReturnType<typeof fetchProfile>>()
const creatorProfile = (address: string): ReturnType<typeof fetchProfile> => {
  let read = creatorProfiles.get(address)
  if (!read) {
    read = fetchProfile(address, { signal: AbortSignal.timeout(3000) }).catch(() => null)
    creatorProfiles.set(address, read)
    void read.then((avatar) => {
      if (!avatar) creatorProfiles.delete(address)
    })
  }
  return read
}

const LIVE: ShellShopServices = {
  client: (signer) => createShopClient(hostApiOptions({ signer })),
  // The creator-sales switch as this document holds it, asked when the listing is mapped, as the read did itself.
  fetchItem: (contract, itemId, client) => fetchBuyListing(contract, itemId, client, () => shopFlags().creatorSales),
  offers: (client) => purchaseOfferSource(client),
  balance: (account, client) => getUsdBalance(account, client),
  resolvers: (client) => liveResolvers(client),
  reader: () => shopChainReader(),
  primeFlags: async () => {
    // With the variants: priming without them would drop an address list for the rest of the document.
    const { flags, variants } = await readRemoteFlags(AbortSignal.timeout(FLAGS_TIMEOUT_MS))
    primeRemoteFlags(flags, variants)
  },
  run: runGroup,
  watch: watchGroup,
  creditRecord: async (buyer, creditIds, client) => {
    const { items } = await fetchUserPurchases(buyer, { all: true, first: 50 }, client)
    const mine = items.filter((p) => creditIds.includes(p.id))
    if (mine.some((p) => p.status === 'SETTLED')) return 'settled'
    return mine.length > 0 && mine.every((p) => p.status === 'EXPIRED') ? 'expired' : 'open'
  },
  creatorName: async (address) => {
    const avatar = await creatorProfile(address)
    const name = avatar?.name?.trim()
    return name ? name.charAt(0).toUpperCase() + name.slice(1) : null
  },
  creatorFace: async (address) => profileFaceUrl(await creatorProfile(address)),
  claim: (id, wait) => claimLock(typeof navigator !== 'undefined' ? navigator.locks : undefined, id, wait),
  manager: (chainId) => trustedCreditsManager(chainId),
  guard: guardedProvider,
  rail: () => ({}),
  storage: defaultStorage,
  sleep: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
  now: () => Date.now(),
  newId: () => crypto.randomUUID(),
  // The site whose node the checkout reads its balance from, whatever deployment the Explorer selected.
  creditsUrl: () => withUtm(checkoutSiteUrl('/shop/credits')),
  namePrice: async (client) => {
    const quote = await quoteNameCredits(client, AbortSignal.timeout(NAME_READ_TIMEOUT_MS))
    return 'quoted' in quote ? quote.quoted : null
  },
  nameStanding: async (name) => {
    const banned = await readBannedNames(AbortSignal.timeout(NAME_READ_TIMEOUT_MS))
    if (banned.some((b) => b.toLowerCase() === name.toLowerCase())) return 'banned'
    // On the chain the gate checked (namesModeFor), never another the land override names.
    const chainId = ethereumChainId()
    return (await checkNameAvailability(name, { signal: AbortSignal.timeout(NAME_READ_TIMEOUT_MS), reader: rpcReader(chainId), chainId })) === 'available' ? 'available' : 'taken'
  },
  namesMode: () => namesModeFor(nameBuild(), { settles: settlementChainId(), ethereum: ethereumChainId() }),
  manaInWorld: () => hostOr('manaInWorld', remoteFlag(REMOTE_FLAG.manaInWorld)) === true,
  manaBalance: (buyer, chainId) => readManaBalanceWei(buyer, chainId, boundedReader(shopChainReader(), MANA_BALANCE_TIMEOUT_MS)),
  manaGuard: manaGuardedProvider,
  manaEvidence: () => createManaEvidence({ reader: shopChainReader() }),
  manaTimeouts: () => ({ sendMs: SEND_MS, readMs: EVIDENCE_READ_MS })
}


/** A watcher signs nothing: the wallet is never asked while a purchase is only followed. */
const NO_WALLET: Eip1193Provider = {
  request: async () => {
    throw new WalletRefusal('Nothing is signed while a purchase is followed.', 4200)
  }
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

/** One checkout group of a quote: its cart lines and offers, what it buys, and one display per unit. */
export interface QuotePart {
  group: CartGroup
  priced: { line: CartLine; offer: PurchaseOffer }[]
  items: ShopLine[]
  displays: ShellPurchaseItem[]
}

/** What the buyer is shown agreeing to: the session it was read for and the quoted terms. */
export interface Quote {
  login: string
  buyer: string
  kind: ShopWalletKind
  session: Session
  client: ShopClient
  /** The groups, in the order they run; none when the Credits can't buy it (and MANA can: `mana`). */
  parts: QuotePart[]
  /** Every unit, in the request's order. */
  items: ShellPurchaseItem[]
  /** Whole credits, all groups together. */
  total: number
  balance: number
  /** Paying in MANA, when the dialog offers it (readQuote). */
  mana?: ManaQuote
  /** The Credits balance couldn't be read (`balance` is 0 then): the Credits can't pay, and MANA does. */
  creditsUnread?: true
  /** Set when the wallet changed account or went away after the terms were shown. */
  stale?: string
}

/**
 * One sale of a purchase paid in Polygon MANA: its group, and the sale built from the group's lines as the chain and the
 * server read them when the terms were read (the guard binds exactly this).
 */
export interface ManaQuotePart {
  part: QuotePart
  terms: ManaSaleTerms
  fingerprint: string
  /** The group's MANA, in wei. */
  price: bigint
}

/**
 * Paying in Polygon MANA, as the buyer is shown it: one sale per group (a cart of the Collection Store's mints and a
 * Marketplace's trades is two), in the order they run, and the buyer's MANA against all of them.
 */
export interface ManaQuote {
  parts: ManaQuotePart[]
  /** Every group's MANA together, in wei. */
  price: bigint
  balance: bigint
  pays: boolean
}

/** A group's run, as the checkout around it reads it. */
type PartOutcome =
  | { kind: 'confirmed' }
  | { kind: 'pending' }
  /**
   * Nothing of the group was handed off, or its transaction reverted. `message` says it as a purchase of
   * its own; `part`, as one transaction of a checkout. `retry`: how it may start again. `switchTo`: the
   * network the buyer's own wallet can send it from, once it is moved there.
   */
  | { kind: 'failed'; message: string; part: string; retry: 'cancelled' | 'failed' | null; topUp?: number; switchTo?: number }

const NOT_BOUGHT = "This purchase didn't go through. Nothing was bought."
/** How a checkout says one of its transactions ended. */
const NEVER_WENT = 'one of its transactions never went out.'
const PART_FAILED = 'one of its transactions didn’t go through.'
/** How a checkout says the rest waited for a part still being confirmed. */
const AFTER_PENDING_PART = 'the rest wasn’t sent while an earlier part was still being confirmed.'
// A reverted transaction's credits are retired, not handed back: they come back with the expiry sweep (as the
// checkout's own reverted failure says), so the copy never claims they were returned.
const REVERTED_PART = 'one of its transactions failed on-chain; the Credits it held come back on their own.'
const REVERTED_ALONE = 'The transaction failed on-chain. Nothing was bought; the Credits it held come back on their own.'
/**
 * The buyer's own wallet would send the purchase, on another network than the one it is on. A self-custody
 * wallet holding the fee money is offered the switch: saying that the fee-free way is unavailable when it was
 * tried first, plainly on a checkout that only sends from the wallet. Any other wallet is told the purchase didn't
 * go out: a managed one, never told of networks or fees, and one short of the fee money, or whose balance can't be
 * read in time. When the relay said why the fee-free way couldn't run (its daily limits: the failure's
 * `feeFreeNote`), that reason is said in place of "Fee-free checkout is unavailable right now".
 */
const switchAfterFeeFree = (chainId: number, note?: string): string =>
  `${note ?? 'Fee-free checkout is unavailable right now.'} You can switch to ${chainLabel(chainId)} and pay the network fee with your wallet. Nothing was bought.`
const switchDirect = (chainId: number): string => `Your wallet is on another network. Switch it to ${chainLabel(chainId)} to send this purchase. Nothing was bought.`
const FEE_FREE_UNAVAILABLE = 'Fee-free checkout is unavailable right now. Nothing was bought. Try again later.'
const NOT_SENT_LATER = 'This purchase couldn’t go out. Nothing was bought. Try again later.'
/**
 * The least of the network's own currency (0.05) a wallet holds to be offered the switch, as the web Shop offers its
 * fee-paying fallback. A threshold for the offer, not an estimate of the fee, which varies: an estimate reverts for
 * reasons that aren't the fee.
 */
const FEE_OFFER_FLOOR_WEI = 50_000_000_000_000_000n
/** How long the balance read may take before the offer is left out: it decides an offer, never the purchase. */
export const FEE_BALANCE_TIMEOUT_MS = 4000
/** How long after its latest send a MANA sale not yet on chain may be retried (./manaFlow). */
export { RETRY_AFTER_MS } from './manaFlow'

/**
 * A confirmed purchase of several groups, as its dialog shows them together: still in progress while any
 * of its operations is, successful once every group was bought, and partly bought if any wasn't. Each
 * operation keeps its own record, lock and watch; this is only what the dialog makes of them.
 */
export interface Checkout {
  ops: Operation[]
  /** Every unit, in the request's order. */
  displays: ShellPurchaseItem[]
  /** How many groups it has: fewer operations than that, and the rest never started. */
  parts: number
  cart: boolean
  /** Why each part of it that wasn't bought wasn't, in the order they ended. */
  failures: string[]
  /** Paid in MANA: what wasn't bought is said to have spent none. */
  mana?: true
  /** The part handed off and not shown on chain that held the rest: while it is set, a step in progress says so. */
  heldBy?: Operation
}

/** An operation and, while its dialog is up, what it shows. */
export type Operation = OperationRecord & {
  store?: CartStore
  watching?: boolean
  unwatchWallet?: () => void
  input?: ShopBuyRequest['input']
  /** This page's hold on the operation (its Web Lock), released once it settles. */
  release?: () => void
  /** Held by another live page: shown, never changed here. */
  foreign?: boolean
  /** When its credit was first read as expired and unused with no receipt for its hash. */
  expiredSince?: number
  /** The checkout of several groups it is one of. */
  checkout?: Checkout
  /** Paid in MANA: the chain, read last, left its nonce unused with none of its sends on its way (RETRY may open). */
  manaRetry?: boolean
  /** Paid in MANA: one of its sends is being signed or sent by this page right now. */
  manaSending?: boolean
  /** Paid in MANA: what its Purchase in progress shows now. */
  manaShown?: 'check' | 'retry' | 'signin' | 'capped'
}

export interface ShellShop {
  /** Answers a `shop.*` request from the app frame. */
  request(method: string, params: unknown): Promise<unknown>
  /** Whether the Shop's dialog is up over the app. */
  dialogOpen(): boolean
}

export function createShellShop(deps: ShellShopDeps): ShellShop {
  const svc: ShellShopServices = { ...LIVE, ...deps.services }
  const ledger = createLedger(svc.storage)
  const operations = new Map<string, Operation>()
  /** The dialog over the app, shared with BUY NAME. */
  const slot = createDialogSlot(deps.ui)
  /** The item operation whose dialog is up; none while it is up for a NAME. */
  const dialogOp = (): Operation | null => {
    const held = slot.holder() as Operation | null
    return held != null && operations.get(held.id) === held ? held : null
  }
  /** A purchase is being opened (its records read, its lock taken): one at a time. */
  let starting = false
  /** A confirmed purchase is running its groups: nothing else opens meanwhile. */
  let running = false
  /** Restores run one after another, so one page never asks twice for a lock it is taking. */
  let restoring: Promise<void> = Promise.resolve()
  /** Other tabs' operations whose lock this page is queued for. */
  const awaiting = new Set<string>()

  const snapshot = (op: Operation): ShopItemOperation => ({
    v: SHOP_PROTOCOL,
    operation: op.id,
    buyer: op.buyer,
    state: op.state,
    items: op.items,
    ...(op.cart ? { cart: true as const } : {}),
    ...(op.next ? { next: op.next } : {}),
    ...(op.txHash !== undefined ? { txHash: op.txHash } : {})
  })
  const keep = (op: Operation): boolean => {
    // Only the page holding an operation writes it while it is unsettled (its end is anyone's to keep).
    if (!op.release && !TERMINAL_STATES.includes(op.state)) return false
    op.updatedAt = svc.now()
    const { store: _s, watching: _w, unwatchWallet: _u, input: _i, release: _r, foreign: _f, expiredSince: _e, checkout: _c, manaRetry: _m, manaSending: _ms, manaShown: _mv, ...record } = op
    return ledger.save(record)
  }
  const publish = (op: Operation): void => {
    keep(op)
    deps.post({ type: SHOP_EVENT, ...snapshot(op) })
  }
  /** A state the operation moves to; a settled one is final. */
  const move = (op: Operation, state: ShopOperationState, extra: Partial<Operation> = {}): void => {
    if (op.foreign || TERMINAL_STATES.includes(op.state)) return
    Object.assign(op, extra, { state })
    publish(op)
    if (TERMINAL_STATES.includes(state)) {
      op.release?.()
      op.release = undefined
    }
  }
  const show = (op: Operation, view: ShellPurchaseView, handlers: ShellPurchaseHandlers): void => slot.show(op, view, handlers, { pointer: op.input === 'pointer' })
  /**
   * The buyer's own wallet as `op` asks it: while it asks the buyer to sign (a signature or a transaction), the dialog up
   * for `op` says so (Confirm in your wallet), and once it answers, the step it showed before comes back, unless another
   * step was shown meanwhile. A managed wallet signs without asking anyone: it is used as it is.
   */
  const prompting = (op: Operation, wallet: Eip1193Provider, kind: ShopWalletKind): Eip1193Provider => promptingWallet(slot, op, wallet, kind)
  const hide = (op: Operation): void => {
    op.unwatchWallet?.()
    op.unwatchWallet = undefined
    slot.hide(op)
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
  /** Requests signed by the current login, while it is `buyer`'s (or any, without one): never as another account. */
  const client = (buyer?: string | null): ShopClient =>
    svc.client(() => {
      const login = deps.login()
      return login && (!buyer || rootAddress(login.identity).toLowerCase() === buyer) ? loginSigner(login) : null
    })
  /** Requests signed by the one login that confirmed the terms, and by no later one. */
  const confirmedClient = (key: string): ShopClient =>
    svc.client(() => {
      const login = deps.login()
      return login && loginKey(login) === key ? loginSigner(login) : null
    })

  const fail = (op: Operation, message: string, retry?: () => void): void => {
    show(op, { step: 'failed', message, retry: retry != null }, { onClose: () => close(op, 'failed'), ...(retry ? { onRetry: retry } : {}) })
  }
  /**
   * A failure the buyer's own wallet gets past from `chainId`: SWITCH TO … AND RETRY moves the wallet there,
   * from the buyer's own click (a wallet refuses a switch it can't tie to one), then tries again as TRY AGAIN
   * does. A declined switch leaves the offer standing; a switch that fails says so.
   */
  const failSwitching = (op: Operation, message: string, chainId: number, wallet: Eip1193Provider, again: () => void): void => {
    let switching = false
    show(op, { step: 'failed', message, retry: true, switchTo: chainLabel(chainId) }, {
      onClose: () => close(op, 'failed'),
      onSwitchNetwork: () => {
        if (switching) return
        switching = true
        requestChainSwitch(wallet, chainId)
          // A wallet may add the network and stay on the one it was on (EIP-3085): only a wallet now on the
          // purchase's network tries again, and any other keeps the offer for another press.
          .then(() => wallet.request({ method: 'eth_chainId' }))
          .then(
            (current) => {
              if (slot.holds(op) && Number(current) === chainId) again()
              else switching = false
            },
            (e: unknown) => {
              switching = false
              if (!slot.holds(op) || isRejection(e)) return
              fail(op, `Your wallet didn’t move to ${chainLabel(chainId)}. Switch it there yourself, then try again.`, again)
            }
          )
      }
    })
  }
  /** Whether `buyer` holds the fee money the switch is offered for on `chainId`: false on a failed or slow read. */
  const feeMoney = (chainId: number, buyer: string): Promise<boolean> =>
    new Promise((resolve) => {
      const timer = setTimeout(() => resolve(false), FEE_BALANCE_TIMEOUT_MS)
      Promise.resolve()
        .then(() => svc.reader().balance(chainId, buyer))
        .then(
          (wei) => wei >= FEE_OFFER_FLOOR_WEI,
          () => false
        )
        .then((funded) => {
          clearTimeout(timer)
          resolve(funded)
        })
    })
  /** Purchase in progress; one paid in MANA shows what it offers now (./manaFlow). */
  const showPending = (op: Operation): void => {
    if (!op.mana) {
      show(op, { step: 'pending' }, { onClose: () => hide(op) })
      return
    }
    manaFlow.showPending(op)
  }
  /** CHECK AGAIN: one look now, besides the watch's own; the dialog follows if it settled. */
  const checkAgain = async (op: Operation, view: (check: 'ready' | 'checking') => void): Promise<void> => {
    if (!op.buyer || op.foreign || !stillPending(op)) return
    view('checking')
    const done = await check(op as Operation & { buyer: string }, (op.store ??= ledger.storeFor(op))).catch(() => false)
    // Repainted only while it is still the purchase in progress: a refusal the watch showed meanwhile stays shown.
    if (!done && slot.holds(op) && stillPending(op)) showPending(op)
  }
  /** Whether the purchase is still in progress (not settled, and not back on a dialog that ends it). */
  const stillPending = (op: Operation): boolean => op.state === 'processing' || op.state === 'pending'
  /** Paying in MANA (./manaFlow), on these operations and through this dialog. */
  const manaFlow = createManaFlow({
    svc,
    ledger,
    slot,
    wallet: () => deps.wallet,
    signedIn,
    loginKey,
    show,
    hide,
    fail,
    close,
    move,
    publish,
    keep,
    checkAgain,
    stillPending,
    // Defined further down: called through, once they are.
    settled: (op, state, extra, message, part) => settled(op, state, extra, message, part),
    restart: (op, req, ended) => restart(op, req, ended),
    reconcile: (op) => reconcile(op),
    check: (op, store) => check(op, store),
    reviewPart: (part, quote) => reviewPart(part, quote),
    create: (req, buyer, items) => create(req, buyer, items),
    prompting: (op, wallet, kind) => prompting(op, wallet, kind),
    presentCheckout: (co) => presentCheckout(co),
    busy: () => running,
    hold: () => {
      running = true
      let held = true
      return () => {
        if (!held) return
        held = false
        running = false
      }
    }
  })
  /** The signed-in login that can buy here: its key held, and a wallet here that signs for its account. */
  const usable = (): { login: StoredLogin; buyer: string } | null => {
    const who = signedIn()
    return who && deps.wallet?.kind(who.buyer) != null && deps.wallet.provider(who.buyer) != null ? who : null
  }
  /** How NAMEs are bought for the login that can buy here, or null: capabilities' `namesMode`, and all shop.buyName admits. */
  const nameSession = (): { buyer: string; login: string; mode: ShopNamesMode } | null => {
    const who = usable()
    const mode = who ? svc.namesMode() : null
    return who && mode ? { buyer: who.buyer, login: loginKey(who.login), mode } : null
  }
  /** BUY NAME (./namePurchase): its own operations, in memory only, in the same dialog as the items'. */
  const names = createNamePurchase({
    session: nameSession,
    client: (key) => confirmedClient(key),
    slot,
    busy: () => starting || running || deps.otherDialogOpen?.() === true,
    post: deps.post,
    openExternal: deps.openExternal,
    services: {
      balance: svc.balance,
      price: svc.namePrice,
      standing: svc.nameStanding,
      sleep: svc.sleep,
      now: svc.now,
      newId: svc.newId,
      creditsUrl: svc.creditsUrl
    }
  })
  /** Purchase successful, with what was bought (the operation's own, or a whole checkout's). */
  const showSuccess = (op: Operation, items: ShellPurchaseItem[] | undefined = op.displays): void => {
    if (!items || items.length === 0) {
      hide(op)
      return
    }
    show(op, { step: 'success', items }, {
      onClose: () => hide(op),
      onBackpack: () => {
        hide(op)
        op.next = 'backpack'
        publish(op)
      }
    })
  }
  /**
   * A watched purchase reached its end: its dialog (or its checkout's), if it is the one up, says so.
   * `part` says how it ended as one transaction of a checkout ("one of its transactions…").
   */
  const settled = (op: Operation, state: 'succeeded' | 'failed' | 'cancelled', extra: Partial<Operation> = {}, message?: string, part?: string): void => {
    move(op, state, extra)
    const co = op.checkout
    if (co) {
      if (state !== 'succeeded') co.failures.push(part ?? (state === 'cancelled' ? NEVER_WENT : PART_FAILED))
      if (dialogOp()?.checkout === co && !running) presentCheckout(co)
      return
    }
    if (!slot.holds(op)) return
    if (state === 'succeeded') showSuccess(op)
    else if (state === 'failed') show(op, { step: 'failed', message: message ?? NOT_BOUGHT, retry: false }, { onClose: () => hide(op) })
    else hide(op)
  }

  /**
   * A checkout of several groups, as its dialog shows it: in progress while any group is, Purchase
   * successful with everything once every group was bought, and otherwise what went and what didn't.
   */
  const presentCheckout = (co: Checkout): void => {
    const anchor = co.ops[co.ops.length - 1]
    if (!anchor) return
    // The part still in progress, shown as itself (CHECK AGAIN, or RETRY for MANA): in a MANA cart it may come before
    // the last, which the executor saw confirmed while the chain hasn't finalized it yet.
    const unsettled = co.ops.find((o) => o.state === 'processing' || o.state === 'pending')
    if (unsettled) {
      showPending(unsettled)
      return
    }
    const bought = co.ops.filter((o) => o.state === 'succeeded').length
    if (co.failures.length === 0 && co.ops.length === co.parts && bought === co.ops.length) {
      showSuccess(anchor, co.displays)
      return
    }
    // Each part's reason is said as one transaction of it; several, as what they add up to.
    const message =
      bought === 0
        ? `None of its transactions went through: nothing was bought${co.mana ? ' and no MANA was spent' : ''}.${co.cart ? ' Everything is still in your cart.' : ''}`
        : partly(co.failures.length === 1 ? co.failures[0] : `more than one of its transactions didn’t go through${co.mana ? '; no MANA was spent for them' : ''}.`, co.cart)
    show(anchor, { step: 'failed', message, retry: false }, { onClose: () => hide(anchor) })
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
    ctx: { buyer: op.buyer!, provider: NO_WALLET, reader: svc.reader(), client: client(op.buyer), walletKind: null, ...svc.rail() },
    store,
    guard: createSpendGuard()
  })

  /** The newer of the checkout's own progress and the record's copy of it. */
  const latest = (a: GroupProgress | null, b: GroupProgress | null): GroupProgress | null => {
    if (!a || !b) return a ?? b
    if (a.updatedAt !== b.updatedAt) return a.updatedAt > b.updatedAt ? a : b
    return RANK[a.status] >= RANK[b.status] ? a : b
  }


  /** One look at an unsettled operation: true once it settled. Anything it can't read is for the next look. */
  const check = async (op: Operation & { buyer: string }, store: CartStore): Promise<boolean> => {
    if (op.mana) return manaFlow.checkMana(op, op.mana)
    const progress = latest(store.getSnapshot().checkout?.groups[0] ?? null, op.progress ?? null)
    const handedOff = op.state === 'pending' || !!op.txHash || !!progress?.txHash || (progress != null && (progress.status === 'submitted' || progress.status === 'pending'))
    if (progress?.status === 'confirmed') {
      settled(op, 'succeeded', { txHash: progress.txHash ?? op.txHash ?? null })
      return true
    }
    if (progress && (progress.status === 'failed' || progress.status === 'expired')) {
      settled(op, 'failed', {}, progress.failure?.message, progress.failure?.message)
      return true
    }
    if (!progress) {
      if (!handedOff) {
        settled(op, 'cancelled')
        return true
      }
      // A hand-off known only by its hash (the checkpoint is gone): its receipt on its chain decides.
      if (op.txHash && op.chainId) {
        const receipt = await svc
          .reader()
          .receipt(op.chainId as Parameters<ChainReader['receipt']>[0], op.txHash)
          .catch(() => null)
        if (receipt?.status === 'success') {
          settled(op, 'succeeded', { txHash: op.txHash })
          return true
        }
        if (receipt?.status === 'reverted') {
          settled(op, 'failed', {}, 'The transaction failed on-chain. Nothing was bought.', REVERTED_PART)
          return true
        }
      }
    } else if (!handedOff) {
      // Reserved, or not even that, and provably not handed off: give the credit back.
      if (progress.status === 'authorizing') {
        const state: PurchaseState = store.getSnapshot().checkout ?? { id: op.id, account: op.buyer, groups: [progress], createdAt: op.createdAt }
        await releaseAuthorizing(watchDeps(op, store), { ...state, groups: [progress] })
      }
      settled(op, 'cancelled')
      return true
    } else {
      const following: GroupProgress =
        progress.status === 'submitted' || progress.status === 'pending' ? progress : { ...progress, status: op.txHash || progress.txHash ? 'pending' : 'submitted', txHash: progress.txHash ?? op.txHash ?? null }
      const result = await svc.watch(watchDeps(op, store), following).catch((): GroupResult => ({ status: 'pending', txHash: following.txHash }))
      if (result.status === 'confirmed') {
        settled(op, 'succeeded', { txHash: result.txHash })
        return true
      }
      if (result.status === 'failed') {
        const reverted = result.failure.kind === 'reverted'
        settled(op, 'failed', {}, reverted ? REVERTED_ALONE : result.failure.message, reverted ? REVERTED_PART : result.failure.message)
        return true
      }
      // A hash with no receipt may have been dropped or replaced: the server's record of the credit
      // itself says whether it was spent. Expired unused (read twice, past the index's lag) means it
      // can no longer be: an expired credit reverts useCredits.
      if (following.txHash && following.creditIds.length > 0) {
        const record = await svc.creditRecord(op.buyer, following.creditIds, client(op.buyer)).catch(() => 'open' as const)
        if (record === 'settled') {
          settled(op, 'succeeded')
          return true
        }
        if (record === 'expired') {
          op.expiredSince ??= svc.now()
          if (svc.now() - op.expiredSince >= EXPIRY_CONFIRM_MS) {
            // Expiry with no consumption indexed past it: it can no longer land, not proof it was never sent.
            settled(op, 'failed', {}, 'This purchase expired before its Credits were spent on chain: nothing was bought.', 'one of its transactions expired before its Credits were spent on chain.')
            return true
          }
        } else op.expiredSince = undefined
      }
    }
    return false
  }

  /**
   * Takes an unsettled operation to its end and never resubmits it. It settles only on authoritative
   * evidence: the checkout's own confirmed or failed state, the executor's watcher (the receipt of the
   * hash, or the server's record of the operation's own credit), or a known hash's receipt on its chain.
   * Anything unknown (an unreadable answer, no receipt yet, a read that fails) stays unsettled and is
   * read again, for as long as the page lives; no timer ends it.
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
      // Read often at first, then every few minutes for as long as the page lives: an unsettled
      // purchase is never left unwatched while this page holds it.
      const slowAfter = svc.now() + WATCH_FOR_MS
      for (let attempt = 0; ; attempt++) {
        try {
          if (await check(op as Operation & { buyer: string }, store)) return
        } catch (e) {
          console.warn('[shell/shop] could not check a purchase; reading it again later', e)
        }
        await svc.sleep(svc.now() >= slowAfter ? WATCH_SLOW_MS : WATCH_BACKOFF_MS[Math.min(attempt, WATCH_BACKOFF_MS.length - 1)])
      }
    } catch {
      // The page is going (its waits refused): nothing else to do here.
    } finally {
      op.watching = false
    }
  }

  /**
   * This buyer's operations from storage that this page may take over: those no live page holds (their
   * page went). An open dialog didn't survive its page; a hand-off is followed. An operation another live
   * page holds (another tab mid-purchase) is only shown, as it stands in storage, and never changed here.
   */
  const restore = (buyer: string): Promise<void> => inTurn(() => adopt(buyer))
  const inTurn = (step: () => Promise<void> | void): Promise<void> => (restoring = restoring.then(step).catch(() => {}))
  const adopt = async (buyer: string): Promise<void> => {
    for (const listed of ledger.load(buyer)) {
      const known = operations.get(listed.id)
      if (known && !known.foreign) continue
      take(buyer, listed, TERMINAL_STATES.includes(listed.state) ? null : await svc.claim(listed.id))
    }
  }

  /**
   * A stored operation as this page now holds it. With its lock (`release`): read again under it (the tab
   * that held it may have moved it on, or ended it, meanwhile), then taken over. Without: another live
   * tab's, shown as it stands, and its lock waited for.
   */
  const take = (buyer: string, listed: OperationRecord, held: (() => void) | null): void => {
    const known = operations.get(listed.id)
    let release = held
    let record: OperationRecord | null = listed
    if (release) {
      record = ledger.saved(buyer, listed.id)
      if (!record || TERMINAL_STATES.includes(record.state)) {
        release()
        release = null
      }
      // Gone from storage: nothing to take over, and never rebuilt from an older read.
      if (!record) return
    }
    const foreign = !TERMINAL_STATES.includes(record.state) && !release
    // Updated in place: a dialog this page shows for it keeps following the same operation.
    const op: Operation = known ?? { ...record }
    const was = known?.state
    Object.assign(op, record, { foreign, release: release ?? undefined })
    operations.set(op.id, op)
    if (known && was !== op.state) {
      // Another tab moved it on: this page's app hears of it, and its dialog, if up, says how it ended.
      deps.post({ type: SHOP_EVENT, ...snapshot(op) })
      if (isTerminal(op.state)) settled(op, op.state)
    }
    if (foreign) queueFor(buyer, op.id)
    if (!release) return
    if (op.state === 'reviewing') move(op, 'cancelled')
    else void reconcile(op)
  }

  /** Queues for another tab's operation's lock, granted once that tab ends the operation or goes. */
  const queueFor = (buyer: string, id: string): void => {
    if (awaiting.has(id)) return
    awaiting.add(id)
    void svc.claim(id, true).then((release) => {
      awaiting.delete(id)
      if (!release) return
      void inTurn(() => {
        // Taken over only for the buyer still signed in here; otherwise left for a later restore.
        const known = operations.get(id)
        if (known?.foreign && signedIn()?.buyer === buyer) take(buyer, known, release)
        else release()
      })
    })
  }

  // ---- Reading the terms and confirming ---------------------------------------------------------

  /** A new operation (for `items`: the request's, or one group of it), held by this page before anything of it is stored. */
  const create = async (req: ShopBuyRequest, buyer: string | null, items: ShopLine[] = req.items): Promise<Operation> => {
    const id = svc.newId()
    const release = (await svc.claim(id)) ?? undefined
    const at = svc.now()
    const op: Operation = { id, buyer, items, ...(req.cart ? { cart: true as const } : {}), state: 'reviewing', createdAt: at, updatedAt: at, release, input: req.input }
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
    starting = true
    void create(req, signedIn()?.buyer ?? null)
      .then((fresh) => {
        fresh.input = op.input
        slot.hand(fresh)
        void review(fresh, req)
      })
      .finally(() => {
        starting = false
      })
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
    // Only the tab that holds the operation runs it.
    if (!op.release) {
      fail(op, "This browser can't keep the purchase to this tab, so it can't run here. You can still buy it on the website.")
      return
    }
    const kind = deps.wallet?.kind(who.buyer) ?? null
    if (!kind || !deps.wallet?.provider(who.buyer)) {
      fail(op, "Your sign-in can't confirm purchases here. Sign in again on the website to buy with Credits.")
      return
    }
    show(op, { step: 'loading', count: unitsOf(req.items) }, { onClose: () => close(op, 'cancelled') })
    let quote: Quote | string
    try {
      quote = await readQuote(op, who.login, who.buyer, kind, req)
    } catch (e) {
      console.warn('[shell/shop] could not read the purchase terms', e)
      if (op.state === 'reviewing' && slot.holds(op)) fail(op, "We couldn't check this item's price. Try again in a moment.", () => restart(op, req, 'cancelled'))
      return
    }
    // Cancelled while it loaded.
    if (op.state !== 'reviewing' || !slot.holds(op)) return
    if (typeof quote === 'string') fail(op, quote)
    else present(op, req, quote)
  }

  const readQuote = async (op: Operation, login: StoredLogin, buyer: string, kind: ShopWalletKind, req: ShopBuyRequest): Promise<Quote | string> => {
    const c = confirmedClient(loginKey(login))
    await svc.primeFlags().catch(() => {})
    // MANA is offered, for one item or a cart, to whoever the one MANA rule shows it to (./manaGate).
    const manaOffered = seesMana({ flag: svc.manaInWorld(), wallet: kind })
    let creditsUnread: unknown = null
    const [listings, balance, manaBalance] = await Promise.all([
      Promise.all(req.items.map((l) => svc.fetchItem(l.contract, l.itemId, c))),
      // Where MANA may pay, an unread Credits balance doesn't stop the purchase: the Credits row says it can't pay.
      manaOffered
        ? svc.balance(buyer, c).catch((e: unknown) => {
            creditsUnread = e
            return null
          })
        : svc.balance(buyer, c),
      manaOffered ? manaFlow.readManaInTime(buyer) : Promise.resolve(null)
    ])
    // One item is "this item"; in a cart, each is named.
    const single = req.items.length === 1
    const named = (i: number): string => (single ? 'This item' : listings[i]?.name || 'One of these items')
    const inputs = listings.map((listing) => (listing ? cartInput(listing) : null))
    const gone = inputs.findIndex((input) => input == null)
    if (gone >= 0) return `${named(gone)} isn't on sale in the Shop right now.`
    for (const [i, l] of req.items.entries()) {
      const available = listings[i]?.available
      if (available != null && l.quantity > available) return available === 0 ? `${named(i)} is sold out.` : `Only ${available} left of ${single ? 'this item' : named(i)}.`
    }
    // The creators' names and pictures, from their profiles, read while the offers are: never the app's word for them.
    const creators = new Map<string, Promise<string | null>>()
    const faces = new Map<string, Promise<string | null>>()
    for (const listing of listings) {
      const creator = listing?.creator
      if (creator && !creators.has(creator)) creators.set(creator, svc.creatorName(creator).catch(() => null))
      if (creator && !faces.has(creator)) faces.set(creator, svc.creatorFace(creator).catch(() => null))
    }
    const account = parseEvmAddress(buyer)
    if (!account) return "Your sign-in can't confirm purchases here."
    // Fresh lines each time the terms are read (a retry included): nothing was handed off before them.
    const store = (op.store = ledger.storeFor(op))
    store.replaceCheckout(null)
    store.clear()
    const ids = req.items.map((l, i) => {
      const added = store.add({ ...inputs[i]!, quantity: l.quantity })
      store.setMethod(added.id, 'credits')
      return added.id
    })
    const stored = store.getSnapshot().lines
    const lines = ids.map((id) => stored.find((l) => l.id === id))
    const short = lines.findIndex((line, i) => line == null || line.quantity !== req.items[i].quantity)
    if (short >= 0) return `${named(short)} can't be bought ${req.items[short].quantity} at a time.`
    const cartLines = lines as CartLine[]
    const session: Session = kind === 'web3' ? { kind: 'web3', wallet: account } : { kind: 'web2', account }
    const offers = await svc.offers(c)(
      cartLines.map((line) => ({ key: line.id, asset: line.asset, listing: line.listing, quantity: line.quantity })),
      session,
      AbortSignal.timeout(15_000)
    )
    const noOffer = cartLines.findIndex((line) => offers[line.id] == null)
    if (noOffer >= 0) return `${named(noOffer)} isn't on sale in the Shop right now.`
    const priced = cartLines.map((line) => ({ line, offer: offers[line.id]! }))
    const partition = partitionCart(priced, session, { creditsBalance: balance?.balanceCents ?? null })
    const refused = partition.refused[0]
    if (refused?.code === 'groupTooLarge') return `That's more than ${MAX_PURCHASE_UNITS} items at once. Buy some of them first.`
    const creditsBuys = !refused && partition.groups.length > 0 && partition.groups.every((g) => g.key.method === 'credits' && g.total.method === 'credits')
    // Weighed on its own: an item the Credits can't buy may still be bought with MANA.
    const mana = manaBalance != null ? await manaFlow.quoteMana(cartLines, offers, session, buyer, c, balance?.balanceCents ?? null, manaBalance) : null
    // An unread Credits balance only stands aside for MANA that pays; otherwise the terms couldn't be read, as before.
    if (!balance && !mana?.pays) throw creditsUnread
    if (!creditsBuys && !mana) {
      return single ? "This item can't be bought with Credits. You can still buy it on the website." : `${named(refused?.index ?? 0)} can't be bought with Credits here. You can still buy it on the website.`
    }
    // Each line's unit price, as the partition charges it (whole credits).
    const unitCredits = new Map<string, number>()
    if (creditsBuys) for (const g of partition.groups) g.lineIds.forEach((id, k) => unitCredits.set(id, g.quotes[k].method === 'credits' ? creditsCeil(g.quotes[k].quoted) : 0))
    const displays = await Promise.all(
      cartLines.map(async (line, i): Promise<ShellPurchaseItem> => {
        const listing = listings[i]!
        const face = listing.creator ? await faces.get(listing.creator) : null
        return {
          name: listing.name,
          creator: (listing.creator ? await creators.get(listing.creator) : null) || shortAddress(listing.creator ?? ''),
          ...(face ? { creatorFace: face } : {}),
          rarity: listing.rarity || 'base',
          thumbnail: listing.thumbnail || undefined,
          ...(listing.isSmart ? { smart: true } : {}),
          category: listing.category === 'emote' ? 'emote' : (listing.wearableCategory ?? listing.category),
          credits: unitCredits.get(line.id) ?? 0,
          key: line.id
        }
      })
    )
    const unitsFor = (i: number): ShellPurchaseItem[] => Array.from({ length: cartLines[i].quantity }, () => displays[i])
    const indexOf = new Map(cartLines.map((line, i) => [line.id, i]))
    const partOf = (group: CartGroup, lines: { line: CartLine; offer: PurchaseOffer }[]): QuotePart => {
      const at = group.lineIds.map((id) => indexOf.get(id)!)
      return { group, priced: at.map((i) => lines[i]), items: at.map((i) => req.items[i]), displays: at.flatMap(unitsFor) }
    }
    const parts: QuotePart[] = creditsBuys ? partition.groups.map((group) => partOf(group, priced)) : []
    const items = cartLines.flatMap((_, i) => unitsFor(i))
    op.displays = items
    const manaQuote: ManaQuote | null = mana
      ? {
          parts: mana.sales.map((sale) => ({ part: partOf(sale.group, mana.priced), terms: sale.terms, fingerprint: sale.fingerprint, price: sale.price })),
          price: mana.price,
          balance: mana.balance,
          pays: mana.pays
        }
      : null
    return {
      login: loginKey(login),
      buyer,
      kind,
      session,
      client: c,
      parts,
      items,
      total: creditsBuys ? creditsCeil(partition.totals.credits.charged) : 0,
      balance: balance?.credits ?? 0,
      ...(balance ? {} : { creditsUnread: true as const }),
      ...(manaQuote ? { mana: manaQuote } : {})
    }
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

  const present = (op: Operation, req: ShopBuyRequest, quote: Quote, initial?: ShopPaymentMethod): void => {
    const short = quote.total - quote.balance
    const creditsPay = quote.parts.length > 0 && short <= 0 && !quote.creditsUnread
    const mana = quote.mana
    // Choose your payment method, as the web Shop's BuyModal shows it: when the MANA pays, or when the buyer holds some
    // that doesn't while the Credits do (the row then says why it can't). Otherwise the Credits' own steps, as before.
    if (mana && (mana.pays || (mana.balance > 0n && creditsPay))) {
      show(
        op,
        {
          step: 'method',
          items: quote.items,
          credits: { price: quote.parts.length > 0 ? quote.total : null, balance: quote.creditsUnread ? null : quote.balance, pays: creditsPay },
          mana: { price: mana.price, balance: mana.balance, pays: mana.pays },
          ...(initial ? { initial } : {})
        },
        { onClose: () => close(op, 'cancelled'), onPay: (method) => void settle(op, req, quote, method) }
      )
      watchWallet(op, quote)
      return
    }
    if (quote.parts.length === 0) {
      fail(op, quote.items.length === 1 ? "This item can't be bought with Credits. You can still buy it on the website." : "Some of these items can't be bought with Credits. You can still buy them on the website.")
      return
    }
    if (short > 0) {
      // Where the buyer holds MANA that can't pay either, Insufficient Funds shows both ways off.
      const neither = mana != null && mana.balance > 0n && !mana.pays
      show(op, { step: 'insufficient', items: quote.items, balance: quote.balance, need: short, ...(neither ? { mana: { price: mana.price, balance: mana.balance } } : {}) }, {
        onClose: () => close(op, 'cancelled'),
        onBuyCredits: () => {
          deps.openExternal(svc.creditsUrl())
          close(op, 'cancelled')
        }
      })
      return
    }
    show(op, { step: 'confirm', items: quote.items, balance: quote.balance }, { onClose: () => close(op, 'cancelled'), onConfirm: () => void settle(op, req, quote) })
    watchWallet(op, quote)
  }

  // ---- Paying -----------------------------------------------------------------------------------

  /**
   * BUY ITEM: the confirmed terms, for the same login and wallet account, through the web Shop's executor,
   * group by group. The first group runs as the operation the dialog opened with; each next one is an
   * operation of its own, held by this page before it runs. A group that fails ends the checkout: what
   * came before it stands (bought, or followed to its end), and what comes after it never starts.
   */
  /** Operations whose BUY ITEM is being acted on: a second press, while the first awaits, does nothing. */
  const confirming = new Set<Operation>()
  const settle = async (op: Operation, req: ShopBuyRequest, quote: Quote, method: ShopPaymentMethod = 'credits'): Promise<void> => {
    if (op.state !== 'reviewing' || !slot.holds(op) || running || confirming.has(op)) return
    // Only a way the terms said pays: the dialog holds BUY otherwise.
    if (method === 'mana' ? !quote.mana?.pays : quote.parts.length === 0 || quote.total > quote.balance || quote.creditsUnread) return
    confirming.add(op)
    try {
      if (method === 'mana') await manaFlow.settleMana(op, req, quote)
      else await settleOnce(op, req, quote)
    } finally {
      confirming.delete(op)
    }
  }
  const settleOnce = async (op: Operation, req: ShopBuyRequest, quote: Quote): Promise<void> => {
    op.unwatchWallet?.()
    // Every TRY AGAIN offered below follows a checkout that handed nothing off.
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
    // Every group settles through a CreditsManager this deployment trusts, in a call the guard reads.
    if (quote.parts.some((p) => !svc.manager(p.group.key.chainId) || p.group.key.call.kind === 'one')) {
      fail(op, "Buying with Credits isn't available here right now.")
      return
    }
    // The wallet's selected account, before anything is reserved for the purchase.
    try {
      await assertWalletAccount(wallet, quote.buyer, quote.kind === 'web3')
    } catch (e) {
      // Cancelled while the wallet answered, its dialog maybe another purchase's by now: nothing to say.
      if (op.state !== 'reviewing' || !slot.holds(op)) return
      fail(op, e instanceof WalletRefusal ? e.message : "Your wallet didn't answer. Try again.", again)
      return
    }
    if (op.state !== 'reviewing' || !slot.holds(op)) return
    // The login may have changed while the wallet answered.
    if (!current()) {
      fail(op, quote.stale ?? 'Your sign-in changed meanwhile. Open the item again to buy it.')
      return
    }
    // A purchase that can't be kept can't be followed after a reload: nothing is reserved without it.
    if (!ledger.writable()) {
      fail(op, STORAGE_REFUSED, again)
      return
    }
    running = true
    // Several groups show as one checkout; one group is its own operation's dialog, as ever.
    const co: Checkout | undefined = quote.parts.length > 1 ? { ops: [], displays: quote.items, parts: quote.parts.length, cart: req.cart === true, failures: [] } : undefined
    // Several groups are all reviewed before any runs, as the web Shop's checkout does: one that can't be
    // bought is refused with nothing spent, never found after another group was paid.
    const reviewed = new Map<QuotePart, Resolved>()
    if (co) {
      show(op, { step: 'processing', balance: quote.balance, count: quote.items.length }, { onClose: () => {} })
      for (const part of quote.parts) {
        let review: Awaited<ReturnType<typeof reviewPart>>
        try {
          review = await reviewPart(part, quote)
        } catch (e) {
          console.warn('[shell/shop] could not review the purchase', e)
          running = false
          fail(op, "We couldn't start this purchase. Nothing was charged.", again)
          return
        }
        if ('why' in review) {
          running = false
          fail(op, review.why, review.retry === 'cancelled' ? again : undefined)
          return
        }
        reviewed.set(part, review.resolved)
      }
    }
    // Followed only once the checkout's dialog is up: a watch that settles at once then updates it.
    const follow: Operation[] = []
    try {
      let last = op
      for (const [i, part] of quote.parts.entries()) {
        let run = op
        if (i > 0) {
          run = await create(req, quote.buyer, part.items)
          slot.hand(run)
        }
        run.items = part.items
        run.displays = part.displays
        if (co) {
          run.checkout = co
          co.ops.push(run)
        }
        last = run
        // A later group's own lock (the first one's was checked when it opened).
        if (i > 0 && !run.release) {
          co!.failures.push("This browser couldn't keep the rest of the purchase to this tab.")
          move(run, 'cancelled')
          break
        }
        const outcome = await runPart(run, part, quote, wallet, current, reviewed.get(part))
        if (outcome.kind === 'pending') {
          follow.push(run)
          // A group handed off but not yet mined holds the rest: signed meanwhile, the next could take the same
          // meta-transaction nonce, and one of the two would revert. What comes after it never starts.
          if (i < quote.parts.length - 1) {
            co!.failures.push(AFTER_PENDING_PART)
            break
          }
        }
        if (outcome.kind !== 'failed') continue
        if (i > 0) {
          // Nothing of this group went out; what came before stands, and what comes after never starts.
          co!.failures.push(outcome.part)
          move(run, 'failed')
          break
        }
        // Nothing of the checkout was handed off.
        if (outcome.topUp != null) present(op, req, { ...quote, balance: outcome.topUp })
        else if (outcome.switchTo != null) failSwitching(op, outcome.message, outcome.switchTo, wallet, () => restart(op, req, 'failed'))
        else fail(op, outcome.message, outcome.retry === 'cancelled' ? again : outcome.retry === 'failed' ? () => restart(op, req, 'failed') : undefined)
        return
      }
      if (co) presentCheckout(co)
      else if (follow.length > 0) showPending(last)
      else showSuccess(last, quote.items)
    } finally {
      running = false
      for (const pending of follow) void reconcile(pending)
    }
  }


  /** A checkout's failure after part of it went out: what went stands, the rest didn't. */
  const partly = (why: string, cart: boolean): string => `Part of this purchase went through, but not all of it: ${why}${cart ? ' What wasn’t bought is still in your cart.' : ''}`

  /**
   * A group's lines as the chain and the server read them now (the sale's live terms), or why it can't be
   * bought, and whether TRY AGAIN may read the terms anew.
   */
  const reviewPart = async (part: QuotePart, quote: Quote): Promise<{ resolved: Resolved } | { why: string; retry: 'cancelled' | null }> => {
    const single = quote.items.length === 1
    const review = await reviewCart(part.priced, quote.session, quote.buyer, svc.resolvers(quote.client))
    const resolved: Resolved = new Map(review.buyable.map((r) => [r.line.id, r]))
    const missing = part.priced.map((p) => p.line).find((l) => !resolved.has(l.id))
    if (!missing) return { resolved }
    const name = single ? 'this item' : missing.display.name
    const why = review.own.length ? `You already own ${name}.` : review.stale.length ? `${single ? 'Its' : `${name}’s`} price changed. Try again to see the new one.` : `${single ? 'This item' : name} can't be bought right now.`
    return { why, retry: review.stale.length ? 'cancelled' : null }
  }

  /**
   * One group, as its own operation, through the executor: never resubmitted, and followed if handed off.
   * `reviewed`: its lines as the checkout already reviewed them, before any group ran.
   */
  const runPart = async (op: Operation, part: QuotePart, quote: Quote, wallet: Eip1193Provider, current: () => boolean, reviewed?: Resolved): Promise<PartOutcome> => {
    const { group } = part
    const chainId = group.key.chainId
    const manager = svc.manager(chainId)
    const call = group.key.call.kind
    if (!manager || call === 'one') return { kind: 'failed', message: "Buying with Credits isn't available here right now.", part: 'buying the rest with Credits isn’t available here right now.', retry: null }
    const refused = (message: string, part: string, retry: 'cancelled' | 'failed' | null, topUp?: number, switchTo?: number): PartOutcome => {
      op.state = 'reviewing'
      publish(op)
      return { kind: 'failed', message, part, retry, ...(topUp != null ? { topUp } : {}), ...(switchTo != null ? { switchTo } : {}) }
    }
    const intent: PurchaseIntent = {
      buyer: quote.buyer,
      chainId,
      manager,
      units: part.items.flatMap((l) => Array.from({ length: l.quantity }, () => ({ contract: l.contract, itemId: l.itemId }))),
      sale: { venue: group.key.contract.toLowerCase(), call },
      current,
      durable: (stage) => durable(op, stage)
    }
    op.chainId = chainId
    move(op, 'processing')
    if (ledger.failed(op.id) || ledger.saved(op.buyer ?? '', op.id)?.state !== 'processing') return refused(STORAGE_REFUSED, 'this browser couldn’t keep a record of the rest of it, so the rest wasn’t sent.', 'cancelled')
    show(op, { step: 'processing', balance: quote.balance, count: quote.items.length }, { onClose: () => {} })
    // The operation's own cart holds its group's lines only (a checkout's first operation read them all).
    const store = (op.store ??= ledger.storeFor(op))
    const lines = part.priced.map((p) => p.line)
    if (store.getSnapshot().lines.length !== lines.length) {
      store.clear()
      store.restore(lines)
    }
    // The relay gets the signed purchase only once its hand-off is kept; refused here, it was never sent,
    // and the answer carries the relay's own evidence of that (`not_sent`).
    const rail = svc.rail()
    const send = rail.fetchImpl ?? ((...a: Parameters<typeof fetch>) => fetch(...a))
    const relay = (async (input: Parameters<typeof fetch>[0], init?: RequestInit) => {
      if ((init?.method ?? 'GET').toUpperCase() === 'POST' && !durable(op, 'handedOff')) {
        return new Response(JSON.stringify({ ok: false, not_sent: true, message: "This browser couldn't keep a record of the purchase, so it wasn't sent." }), {
          status: 409,
          headers: { 'content-type': 'application/json' }
        })
      }
      return send(input, init)
    }) as typeof fetch
    let unmirror = (): void => {}
    let result: GroupResult
    try {
      const review = reviewed ? { resolved: reviewed } : await reviewPart(part, quote)
      if ('why' in review) return refused(review.why, review.why, review.retry)
      const { resolved } = review
      store.startCheckout([group])
      // Every step the executor writes to the checkout is copied into the operation's record.
      const mirror = (): void => {
        const progress = store.getSnapshot().checkout?.groups[0]
        if (!progress) return
        op.progress = { ...progress }
        keep(op)
      }
      mirror()
      unmirror = store.subscribe(mirror)
      result = await svc.run(
        {
          ctx: { buyer: quote.buyer, provider: svc.guard(prompting(op, wallet, quote.kind), intent), reader: svc.reader(), client: quote.client, walletKind: quote.kind, ...rail, fetchImpl: relay },
          store,
          guard: createSpendGuard()
        },
        group,
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
        return { kind: 'pending' }
      }
      return refused("We couldn't start this purchase. Nothing was charged.", 'the rest of it couldn’t start.', 'cancelled')
    }
    switch (result.status) {
      case 'confirmed':
        move(op, 'succeeded', { txHash: result.txHash })
        return { kind: 'confirmed' }
      case 'pending':
        move(op, 'pending', { txHash: result.txHash })
        return { kind: 'pending' }
      case 'failed': {
        // The balance fell short between the quote and the authorization: Insufficient Funds after all.
        if (result.topUp) return refused(result.failure.message, 'your Credits balance didn’t cover the rest of it.', null, creditsFloor(result.topUp.balanceCents as UsdCents))
        // The wallet's own transaction found the wallet on another network. Only a self-custody wallet holding
        // the fee money is offered the switch (a managed one's balance isn't even read).
        const required = result.failure.switchTo
        if (required != null) {
          const feeFree = result.failure.feeFreeUnavailable === true
          const funded = quote.kind === 'web3' && (await feeMoney(required, quote.buyer))
          if (funded) {
            return refused(feeFree ? switchAfterFeeFree(required, result.failure.feeFreeNote) : switchDirect(required), `the rest of it needs your wallet on ${chainLabel(required)} to pay the network fee.`, 'failed', undefined, required)
          }
          const note = result.failure.feeFreeNote
          return refused(feeFree ? (note != null ? `${note} Nothing was bought.` : FEE_FREE_UNAVAILABLE) : NOT_SENT_LATER, NEVER_WENT, 'failed')
        }
        // A failed group handed nothing off, or its transaction reverted: either way this attempt is over.
        return refused(result.failure.message, result.failure.kind === 'reverted' ? REVERTED_PART : result.failure.message, result.failure.kind !== 'refused' ? 'failed' : null)
      }
      case 'skipped':
        return refused("This purchase couldn't run. Nothing was charged.", 'the rest of it couldn’t run.', 'cancelled')
    }
  }

  // ---- Requests ---------------------------------------------------------------------------------

  const buy = async (params: unknown): Promise<ShopOperation> => {
    const req = parseBuyRequest(params)
    if (starting || running || slot.open() || deps.otherDialogOpen?.()) throw shopBusy()
    starting = true
    try {
      const who = signedIn()
      if (who) {
        await restore(who.buyer)
        // An item while a purchase of it is unsettled, here or in another tab: that purchase, never a
        // second one. This page follows its own; another tab's follows itself.
        const unsettled = [...operations.values()].find(
          (o) => o.buyer === who.buyer && (o.state === 'processing' || o.state === 'pending') && o.items.some((l) => req.items.some((r) => sameItem(l, r)))
        )
        if (unsettled) {
          unsettled.input = req.input
          showPending(unsettled)
          if (!unsettled.foreign) void reconcile(unsettled)
          return snapshot(unsettled)
        }
      }
      const op = await create(req, who?.buyer ?? null)
      // Its first step (sign in, loading, or why it can't) shows before this returns.
      void review(op, req)
      return snapshot(op)
    } finally {
      starting = false
    }
  }

  const capabilities = (): ShopCapabilities => {
    const login = deps.login()
    if (!login || loginExpired(login)) return { v: SHOP_PROTOCOL, buyer: null, wallet: null, credits: false, reason: 'signedOut', codedErrors: true }
    const buyer = rootAddress(login.identity).toLowerCase()
    if (login.guest) return { v: SHOP_PROTOCOL, buyer, wallet: null, credits: false, reason: 'guest', codedErrors: true }
    const wallet = deps.wallet?.kind(buyer) ?? null
    const canPay = wallet != null && deps.wallet?.provider(buyer) != null && !!login.identity.ephemeralIdentity.privateKey
    const marketplace = canPay && wallet === 'web3' && deps.wallet?.marketplace?.(buyer) === true
    const namesMode = canPay ? nameSession()?.mode : undefined
    // What the HUD shows of MANA (the MANA equivalent of a price) follows the same rule as the purchase (./manaGate).
    const mana = canPay && seesMana({ flag: svc.manaInWorld(), wallet })
    return {
      v: SHOP_PROTOCOL,
      buyer,
      wallet,
      credits: canPay,
      marketplace,
      ...(namesMode ? { namesMode } : {}),
      ...(mana ? { mana: true } : {}),
      ...(canPay ? {} : { reason: 'noWallet' as const }),
      codedErrors: true
    }
  }

  /** The flags file, read once for this page's capabilities (each purchase reads it again as it opens). */
  let flagsRead: Promise<void> | null = null
  const readFlagsOnce = (): Promise<void> =>
    (flagsRead ??= svc.primeFlags().catch(() => {
      flagsRead = null
    }))

  const balance = async (): Promise<ShopBalance | null> => {
    const who = signedIn()
    if (!who) return null
    const b = await svc.balance(who.buyer, client(who.buyer))
    return { v: SHOP_PROTOCOL, credits: b.credits }
  }

  /** One operation by id; with no id, the signed-in buyer's, newest first. */
  const status = async (params: unknown): Promise<ShopOperation | ShopOperation[] | null> => {
    const who = signedIn()
    if (who) await restore(who.buyer)
    const empty = params == null || (typeof params === 'object' && Object.keys(params as object).length === 0)
    if (empty) {
      if (!who) return []
      // Newest first, the NAMEs' among the items'; the later of two made in the same millisecond first too
      // (the sort is stable).
      const mine = [...operations.values()].filter((o) => o.buyer === who.buyer).map((o) => ({ ...snapshot(o), createdAt: o.createdAt }))
      return [...mine, ...names.list(who.buyer)]
        .reverse()
        .sort((a, b) => b.createdAt - a.createdAt)
        .map(({ createdAt: _c, ...op }) => op)
    }
    const id = parseOperationRef(params)
    const op = operations.get(id)
    // Only the signed-in buyer's own.
    if (!who) return null
    return op && op.buyer === who.buyer ? snapshot(op) : names.get(id, who.buyer)
  }

  return {
    async request(method, params) {
      switch (method) {
        case SHOP_METHODS.capabilities: {
          const who = signedIn()
          if (who) await restore(who.buyer)
          // The MANA rule reads the flags file: it is read before the answer, once for the page.
          await readFlagsOnce()
          return capabilities()
        }
        case SHOP_METHODS.balance:
          return balance()
        case SHOP_METHODS.buy:
          return buy(params)
        case SHOP_METHODS.buyName:
          return names.buy(params)
        case SHOP_METHODS.status:
          return status(params)
        default:
          throw new Error(`unknown request ${method}`)
      }
    },
    // A purchase still starting (before its dialog shows) counts: Donate must not open over it.
    dialogOpen: () => starting || running || slot.holder() != null
  }
}
