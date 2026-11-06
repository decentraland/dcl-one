// The web shell's side of the in-world Shop purchase (shell/shop/purchase): what it shows the buyer, when
// it signs, for whom, what it tells the HUD, and how it follows a purchase that outlives its dialog. The
// Shop's reads, the executor and its watcher are fakes here (shellShopRails runs the real ones); the cart
// partition, review and the per-operation ledger are the real ones.
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Eip1193Provider } from '../../data/auth/wallet'
import type { CartGroup } from '../../shop/cart/partition'
import { createFakeWallet, creatorTrade, type FakeWallet } from '../../shop/checkout/fakes'
import type { CheckoutDeps, GroupResult } from '../../shop/checkout/purchase/executor'
import type { CatalogItem } from '../../shop/discovery/lib/catalogItem'
import type { UsdCents } from '../../shop/model/money'
import type { GroupProgress } from '../../shop/model/purchase'
import { BOOK, creatorOrder, ITEM, storeMint } from '../../shop/payment/fixtures'
import type { StoredLogin } from '../features/auth/sso'
import { SHOP_EVENT, SHOP_PROTOCOL, type ShopOperation, type ShopOperationEvent } from '../lib/shopProtocol'
import { claimLock, createShellShop, type ShellPurchaseHandlers, type ShellPurchaseView, type ShellShopServices, type ShellShopWallet } from '../shell/shop/purchase'
import { createLedger, type OperationRecord } from '../shell/shop/ledger'
import type { PurchaseIntent } from '../shell/shop/walletGuard'

const BUYER = BOOK.buyer.toLowerCase()
const OTHER = '0x' + '9'.repeat(40)
const CONTRACT = BOOK.collection.toLowerCase()
const TRUSTED = { chainId: 137, address: '0x' + 'ce'.repeat(20), name: 'Decentraland Credits', version: '1.0.0' }
/** A transaction hash, as the chain gives one. */
const TX = '0x' + 'ab'.repeat(32)

const EXPIRES = new Date(Date.now() + 3_600_000).toISOString()
const logins = new Map<string, StoredLogin>()

/** The login the shell holds: one object per account, as shell/login.ts keeps it. */
function login(account = BUYER, extra: Partial<StoredLogin> = {}): StoredLogin {
  const key = account + JSON.stringify(extra)
  const held = logins.get(key)
  if (held) return held
  const made: StoredLogin = {
    address: account,
    identity: {
      ephemeralIdentity: { address: '0x' + '1'.repeat(40), publicKey: '0x04', privateKey: '0x' + 'ab'.repeat(32) },
      expiration: EXPIRES,
      authChain: [
        { type: 'SIGNER', payload: account, signature: '' },
        { type: 'ECDSA_EPHEMERAL', payload: 'Decentraland Login', signature: '0x' }
      ]
    },
    ...extra
  }
  logins.set(key, made)
  return made
}

const LISTING = {
  id: 'listing',
  name: 'Golden Armour',
  creator: BOOK.creator,
  contractAddress: CONTRACT,
  itemId: String(ITEM),
  chainId: 137,
  rarity: 'epic',
  thumbnail: 'https://peer.example/thumb.png',
  category: 'wearable',
  wearableCategory: 'upper_body',
  urn: 'urn:decentraland:matic:collections-v2:x:7',
  available: 10
} as unknown as CatalogItem

/** A Storage in memory, shared between two shells to stand for a reload. */
function memoryStorage(): Storage {
  const map = new Map<string, string>()
  return {
    get length() {
      return map.size
    },
    clear: () => map.clear(),
    getItem: (k) => map.get(k) ?? null,
    key: (i) => [...map.keys()][i] ?? null,
    removeItem: (k) => void map.delete(k),
    setItem: (k, v) => void map.set(k, v)
  }
}

/** What the executor writes to the group's checkout before it answers `pending`: handed off, a hash. */
function handedOff(deps: CheckoutDeps, group: CartGroup, txHash = TX): void {
  deps.store.dispatch(group.keyId, { type: 'authorize' })
  deps.store.dispatch(group.keyId, { type: 'authorized', creditIds: ['0x' + 'a1'.repeat(32)] })
  deps.store.dispatch(group.keyId, { type: 'submit' })
  deps.store.dispatch(group.keyId, { type: 'hash', txHash })
  deps.store.dispatch(group.keyId, { type: 'observed' })
}

type Shown = { view: ShellPurchaseView; handlers: ShellPurchaseHandlers }

/** Operation ids, unique across every shell (two tabs never make the same one). */
let opIds = 0

/** Each test's shells, stopped when it ends: a watcher reads an unsettled purchase for as long as its page lives. */
const pages: Array<() => void> = []
afterEach(() => {
  for (const close of pages.splice(0)) close()
})
type RunFn = (deps: CheckoutDeps, group: CartGroup) => Promise<GroupResult>

function setup(
  opts: {
    balanceCents?: number
    wallet?: ShellShopWallet | null
    account?: string
    login?: () => StoredLogin | null
    run?: RunFn
    watch?: (deps: CheckoutDeps, progress: GroupProgress) => Promise<GroupResult>
    listing?: CatalogItem | null
    /** Shared between two shells for a reload; a fresh one in memory by default; null for none at all. */
    storage?: Storage | null
    manager?: typeof TRUSTED | null
    /** How far the clock moves per watcher wait (the wait itself, by default). */
    tick?: number
    /** The chain's receipts by hash (a missing hash has none yet). */
    receipts?: Record<string, 'success' | 'reverted'>
    /** The creator's profile name; Aurora by default. */
    creatorName?: ShellShopServices['creatorName']
    /** This tab's Web Locks (`locks().tab`); every lock is free by default. */
    claim?: ShellShopServices['claim']
    /** The server's record of the operation's credits; still open by default. */
    creditRecord?: ShellShopServices['creditRecord']
    /** Listings by item id (for several items); `listing` for every id by default. */
    items?: Record<string, CatalogItem>
    /** The server's offers; the store's mint of item 7 for every line by default. */
    offers?: ShellShopServices['offers']
    resolvers?: ShellShopServices['resolvers']
  } = {}
) {
  const shown: Shown[] = []
  const events: ShopOperationEvent[] = []
  const provider: FakeWallet = createFakeWallet({ account: opts.account ?? BUYER })
  const wallet: ShellShopWallet | null = opts.wallet === undefined ? { kind: vi.fn(() => 'managed' as const), provider: vi.fn(() => provider) } : opts.wallet
  const run = vi.fn<RunFn>(opts.run ?? (async () => ({ status: 'confirmed', txHash: TX })))
  const watch = vi.fn<(deps: CheckoutDeps, progress: GroupProgress) => Promise<GroupResult>>(opts.watch ?? (async () => ({ status: 'pending', txHash: TX })))
  const guard = vi.fn((p: Eip1193Provider, _intent: PurchaseIntent) => p)
  const cents = (opts.balanceCents ?? 7000) as UsdCents
  const storage = opts.storage === undefined ? memoryStorage() : opts.storage
  const creditRecord = vi.fn<ShellShopServices['creditRecord']>(opts.creditRecord ?? (async () => 'open'))
  /** Each client's signer, in the order the shell made them (the quote's is the first after shop.buy). */
  const signers: Array<() => unknown> = []
  /** The credits the shell gave back (the salts of each cancel). */
  const cancels: string[][] = []
  /** Every wait the watcher asked for, in ms. */
  const sleeps: number[] = []
  let clock = 1_000_000
  let closed = false
  pages.push(() => (closed = true))
  const services: Partial<ShellShopServices> = {
    client: (signer) => {
      signers.push(signer)
      return {
        authorizations: {
          cancel: async ({ salts }: { salts: string[] }) => {
            cancels.push(salts)
            return { released: salts.length }
          }
        }
      } as never
    },
    fetchItem: async (_contract, itemId) => opts.items?.[itemId] ?? (opts.listing === undefined ? LISTING : opts.listing),
    offers: opts.offers ?? (() => async (queries, session) => Object.fromEntries(queries.map((q) => [q.key, storeMint(session)]))),
    balance: async () => ({ balanceCents: cents, credits: Math.floor(cents / 10) }),
    resolvers: opts.resolvers ?? (() => ({ trade: async () => null, store: async () => ({ priceWei: '2300000000000000000', available: 10 }) })),
    reader: () =>
      ({
        receipt: vi.fn(async (_chain: number, hash: string) => {
          const status = opts.receipts?.[hash]
          return status ? { transactionHash: hash, status, blockNumber: 1n } : null
        })
      }) as never,
    primeFlags: async () => {},
    run: run as unknown as ShellShopServices['run'],
    watch,
    manager: () => (opts.manager === undefined ? TRUSTED : opts.manager),
    guard,
    storage: () => storage,
    // The clock moves with each wait, and the wait yields, so a test can change what the watcher sees.
    sleep: async (ms) => {
      if (closed) throw new Error('the page is gone')
      sleeps.push(ms)
      clock += opts.tick ?? ms
      await new Promise((resolve) => setTimeout(resolve, 2))
    },
    now: () => clock,
    newId: () => `0f0e0d0c-0000-4000-8000-${String(++opIds).padStart(12, '0')}`,
    creditsUrl: () => 'https://decentraland.example/shop/credits',
    creatorName: opts.creatorName ?? (async () => 'Aurora'),
    claim: opts.claim ?? (async () => () => {}),
    creditRecord
  }
  const deps = {
    login: opts.login ?? (() => login()),
    wallet,
    ui: { show: vi.fn((view: ShellPurchaseView, handlers: ShellPurchaseHandlers, _opening?: { pointer: boolean }) => void shown.push({ view, handlers })), hide: vi.fn() },
    post: vi.fn((e: ShopOperationEvent) => void events.push(e)),
    openExternal: vi.fn(),
    signIn: vi.fn(),
    services
  }
  const shop = createShellShop(deps)
  const last = (): Shown => shown[shown.length - 1]
  const until = async (step: ShellPurchaseView['step']): Promise<Shown> => {
    await vi.waitFor(() => expect(last()?.view.step).toBe(step))
    return last()
  }
  const buy = (itemId = String(ITEM)) => shop.request('shop.buy', { v: SHOP_PROTOCOL, items: [{ contract: CONTRACT, itemId, quantity: 1 }], method: 'credits' }) as Promise<ShopOperation>
  /** CHECKOUT: the cart's lines, as `itemId` or `itemId×quantity`. */
  const checkout = (...lines: string[]) =>
    shop.request('shop.buy', {
      v: SHOP_PROTOCOL,
      items: lines.map((l) => ({ contract: CONTRACT, itemId: l.split('x')[0], quantity: Number(l.split('x')[1] ?? 1) })),
      method: 'credits',
      cart: true
    }) as Promise<ShopOperation>
  const states = (): string[] => events.map((e) => e.state)
  return { shop, deps, shown, events, run, watch, guard, provider, wallet, last, until, buy, checkout, states, signers, creditRecord, cancels, sleeps }
}

/**
 * Web Locks as one browser holds them across its tabs: one holder per name, until it lets go or its tab
 * goes, and then the next tab queued for it.
 */
function locks() {
  const held = new Map<string, string>()
  const queued = new Map<string, Array<{ tab: string; grant: (release: () => void) => void }>>()
  const releaser = (id: string, tab: string) => () => {
    if (held.get(id) !== tab) return
    held.delete(id)
    const next = queued.get(id)?.shift()
    if (!next) return
    held.set(id, next.tab)
    next.grant(releaser(id, next.tab))
  }
  return {
    tab:
      (name: string): ShellShopServices['claim'] =>
      async (id, wait) => {
        if (held.has(id)) {
          if (!wait) return null
          return new Promise((grant) => queued.set(id, [...(queued.get(id) ?? []), { tab: name, grant }]))
        }
        held.set(id, name)
        return releaser(id, name)
      },
    close(name: string): void {
      for (const list of queued.values()) for (let i = list.length - 1; i >= 0; i--) if (list[i].tab === name) list.splice(i, 1)
      for (const [id, tab] of [...held]) if (tab === name) releaser(id, tab)()
    },
    holds: (id: string): string | undefined => held.get(id)
  }
}

describe('the shell purchase', () => {
  it('answers shop.buy at once with the operation, then shows the item, its price and the balance', async () => {
    const t = setup()
    const op = await t.buy()
    expect(op).toMatchObject({ state: 'reviewing', items: [{ contract: CONTRACT, itemId: '7', quantity: 1 }] })
    expect(t.shown[0].view.step).toBe('loading')
    // No hint from the app: the dialog's first focus rings as usual.
    expect(t.deps.ui.show.mock.calls[0][2]).toEqual({ pointer: false })
    const { view } = await t.until('confirm')
    expect(view).toMatchObject({ step: 'confirm', balance: 700, items: [{ name: 'Golden Armour', creator: 'Aurora', rarity: 'epic', category: 'upper_body' }] })
    expect(t.run).not.toHaveBeenCalled()
  })

  it('pays on BUY ITEM for the login’s account, through the wallet bound to the confirmed purchase', async () => {
    const t = setup()
    await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    await t.until('success')
    expect(t.run).toHaveBeenCalledTimes(1)
    const [deps] = t.run.mock.calls[0]
    expect(deps.ctx).toMatchObject({ buyer: BUYER, walletKind: 'managed' })
    const [provider, intent] = t.guard.mock.calls[0]
    expect(provider).toBe(t.provider)
    expect(intent).toMatchObject({ buyer: BUYER, chainId: 137, manager: TRUSTED, units: [{ contract: CONTRACT, itemId: '7' }], sale: { venue: BOOK.store.contract.toLowerCase(), call: 'storeBuy' } })
    expect(intent.current()).toBe(true)
    // The wallet's account was read before anything was reserved.
    expect(t.provider.requests[0].method).toBe('eth_accounts')
    expect(t.states()).toEqual(['reviewing', 'processing', 'succeeded'])
    expect(t.events.every((e) => e.type === SHOP_EVENT && e.v === SHOP_PROTOCOL)).toBe(true)
  })

  it('sends the HUD to the Backpack from the success dialog', async () => {
    const t = setup()
    await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    ;(await t.until('success')).handlers.onBackpack?.()
    expect(t.deps.ui.hide).toHaveBeenCalled()
    expect(t.events[t.events.length - 1]).toMatchObject({ state: 'succeeded', next: 'backpack' })
  })

  it('shows Insufficient Funds when the balance is short, and BUY CREDITS opens the credit packs', async () => {
    const t = setup({ balanceCents: 100 })
    await t.buy()
    const { view, handlers } = await t.until('insufficient')
    expect(view).toMatchObject({ step: 'insufficient', balance: 10 })
    handlers.onBuyCredits?.()
    expect(t.deps.openExternal).toHaveBeenCalledWith('https://decentraland.example/shop/credits')
    expect(t.states()).toEqual(['reviewing', 'cancelled'])
    expect(t.run).not.toHaveBeenCalled()
  })

  it('cancels without signing', async () => {
    const t = setup()
    await t.buy()
    ;(await t.until('confirm')).handlers.onClose()
    expect(t.states()).toEqual(['reviewing', 'cancelled'])
    expect(t.run).not.toHaveBeenCalled()
  })

  it('asks a signed-out buyer, or a guest, to sign in', async () => {
    for (const who of [() => null, () => login(BUYER, { guest: true })]) {
      const t = setup({ login: who })
      await t.buy()
      ;(await t.until('signIn')).handlers.onSignIn?.()
      expect(t.deps.signIn).toHaveBeenCalled()
      expect(t.states().at(-1)).toBe('cancelled')
    }
  })

  it('says so when no wallet here can sign for the login', async () => {
    const t = setup({ wallet: { kind: () => null, provider: () => null } })
    await t.buy()
    expect((await t.until('failed')).view).toMatchObject({ retry: false })
  })

  it('refuses to pay when the wallet is on another account, before reserving anything', async () => {
    const t = setup({ account: OTHER })
    await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    const { view } = await t.until('failed')
    expect(view.step === 'failed' && view.message).toMatch(/Switch it to/)
    expect(t.run).not.toHaveBeenCalled()
  })

  it('drops the terms when the wallet switches account while they are shown', async () => {
    const t = setup()
    await t.buy()
    const confirm = await t.until('confirm')
    t.provider.emit('accountsChanged', [OTHER])
    t.provider.account = BUYER
    confirm.handlers.onConfirm?.()
    const { view } = await t.until('failed')
    expect(view.step === 'failed' && view.message).toMatch(/changed account/)
    expect(t.run).not.toHaveBeenCalled()
  })

  it('refuses to pay when the login changed between the quote and BUY ITEM', async () => {
    let account = BUYER
    const t = setup({ login: () => login(account) })
    await t.buy()
    const confirm = await t.until('confirm')
    account = OTHER
    confirm.handlers.onConfirm?.()
    const { view } = await t.until('failed')
    expect(view.step === 'failed' && view.message).toMatch(/sign-in changed/)
    expect(t.run).not.toHaveBeenCalled()
  })

  it('refuses to pay when the sign-in is renewed while the wallet answers, even for the same account', async () => {
    let held = login()
    const t = setup({ login: () => held })
    const answer = t.provider.request.bind(t.provider)
    t.provider.request = async (args) => {
      // The account read comes back for the buyer, but meanwhile the login was renewed.
      if (args.method === 'eth_accounts') held = { ...held, identity: { ...held.identity, expiration: new Date(Date.now() + 7_200_000).toISOString() } }
      return answer(args)
    }
    await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    const { view } = await t.until('failed')
    expect(view.step === 'failed' && view.message).toMatch(/sign-in changed/)
    expect(t.run).not.toHaveBeenCalled()
  })

  it('reserves nothing when this browser can’t keep the purchase', async () => {
    const t = setup({ storage: null })
    await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    const { view } = await t.until('failed')
    expect(view.step === 'failed' && view.message).toMatch(/can't keep a record/)
    expect(t.run).not.toHaveBeenCalled()
  })

  it('reserves nothing when the purchase’s first record doesn’t stick', async () => {
    const storage = memoryStorage()
    const write = storage.setItem.bind(storage)
    storage.setItem = (key, value) => {
      if (key.includes(':record:') && value.includes('"processing"')) throw new DOMException('full', 'QuotaExceededError')
      write(key, value)
    }
    const t = setup({ storage })
    await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    await t.until('failed')
    expect(t.run).not.toHaveBeenCalled()
  })

  it('pays nothing without a CreditsManager this deployment trusts', async () => {
    const t = setup({ manager: null })
    await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    await t.until('failed')
    expect(t.run).not.toHaveBeenCalled()
  })

  it('follows a handed-off purchase to its end after the dialog, never resubmitting it', async () => {
    let settled = false
    const t = setup({
      run: async (deps, group) => {
        handedOff(deps, group)
        return { status: 'pending', txHash: TX }
      },
      watch: async () => (settled ? { status: 'confirmed', txHash: TX } : { status: 'pending', txHash: TX })
    })
    await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    ;(await t.until('pending')).handlers.onClose()
    await vi.waitFor(() => expect(t.watch).toHaveBeenCalled())
    settled = true
    await vi.waitFor(() => expect(t.states().at(-1)).toBe('succeeded'))
    expect(t.run).toHaveBeenCalledTimes(1)
  })

  it('shows the unsettled purchase again instead of starting a second one of the same item', async () => {
    const t = setup({
      run: async (deps, group) => {
        handedOff(deps, group)
        return { status: 'pending', txHash: TX }
      }
    })
    const first = await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    ;(await t.until('pending')).handlers.onClose()
    const again = await t.buy()
    expect(again.operation).toBe(first.operation)
    expect(t.last().view.step).toBe('pending')
    expect(t.run).toHaveBeenCalledTimes(1)
    // Another item is another purchase.
    t.last().handlers.onClose()
    await expect(t.buy('8')).resolves.toMatchObject({ state: 'reviewing' })
  })

  it('keeps answering shop.status for earlier operations', async () => {
    const t = setup()
    const first = await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    ;(await t.until('success')).handlers.onClose()
    const second = await t.buy('8')
    expect(second.operation).not.toBe(first.operation)
    expect(await t.shop.request('shop.status', { operation: first.operation })).toMatchObject({ state: 'succeeded' })
    const list = (await t.shop.request('shop.status', {})) as ShopOperation[]
    expect(list.map((o) => o.operation)).toEqual([second.operation, first.operation])
  })

  it('restores an unsettled purchase after a reload and watches it to its end', async () => {
    const storage = memoryStorage()
    const before = setup({
      storage,
      run: async (deps, group) => {
        handedOff(deps, group)
        return { status: 'pending', txHash: TX }
      }
    })
    const op = await before.buy()
    ;(await before.until('confirm')).handlers.onConfirm?.()
    await before.until('pending')
    // The page reloads: a new shell over the same storage.
    const after = setup({ storage, watch: async () => ({ status: 'confirmed', txHash: TX }) })
    expect(await after.shop.request('shop.status', { operation: op.operation })).toMatchObject({ operation: op.operation })
    await vi.waitFor(() => expect(after.events.find((e) => e.operation === op.operation && e.state === 'succeeded')).toBeTruthy())
    expect(after.watch).toHaveBeenCalled()
    expect(after.run).not.toHaveBeenCalled()
  })

  it('watches, after a reload, a handed-off purchase whose checkout was never written', async () => {
    const storage = memoryStorage()
    const write = storage.setItem.bind(storage)
    storage.setItem = (key, value) => {
      if (key.includes('dcl-shop-checkout')) throw new DOMException('full', 'QuotaExceededError')
      write(key, value)
    }
    const before = setup({
      storage,
      run: async (deps, group) => {
        handedOff(deps, group)
        return { status: 'pending', txHash: TX }
      }
    })
    const op = await before.buy()
    ;(await before.until('confirm')).handlers.onConfirm?.()
    await before.until('pending')
    const after = setup({ storage, watch: async () => ({ status: 'confirmed', txHash: TX }) })
    await after.shop.request('shop.capabilities', undefined)
    await vi.waitFor(() => expect(after.events.find((e) => e.operation === op.operation)?.state).toBe('succeeded'))
    expect(after.events.some((e) => e.operation === op.operation && e.state === 'cancelled')).toBe(false)
  })

  it('never gives back the credit of a handed-off purchase whose checkout stopped at its reservation', async () => {
    const storage = memoryStorage()
    const write = storage.setItem.bind(storage)
    let checkoutWrites = 0
    storage.setItem = (key, value) => {
      // The checkout keeps its reservation; every later checkout write is lost.
      if (key.includes('dcl-shop-checkout') && ++checkoutWrites > 2) throw new DOMException('full', 'QuotaExceededError')
      write(key, value)
    }
    const before = setup({
      storage,
      run: async (deps, group) => {
        handedOff(deps, group)
        return { status: 'pending', txHash: TX }
      }
    })
    const op = await before.buy()
    ;(await before.until('confirm')).handlers.onConfirm?.()
    await before.until('pending')
    let watched: GroupProgress | null = null
    const after = setup({
      storage,
      watch: async (_deps, progress) => {
        watched = progress
        return { status: 'confirmed', txHash: TX }
      }
    })
    await after.shop.request('shop.capabilities', undefined)
    await vi.waitFor(() => expect(after.events.find((e) => e.operation === op.operation)?.state).toBe('succeeded'))
    expect(watched).toMatchObject({ status: 'pending', txHash: TX })
  })

  /** A record as an earlier page left it, with no checkout: only its own fields. */
  function recorded(storage: Storage, record: Record<string, unknown>): string {
    const id = '0f0e0d0c-0000-4000-8000-00000000abcd'
    storage.setItem(`dcl-inworld-shop:record:v1:${BUYER}:${id}`, JSON.stringify({ id, buyer: BUYER, items: [{ contract: CONTRACT, itemId: '7', quantity: 1 }], createdAt: 1, updatedAt: 1, ...record }))
    return id
  }

  it('never fails a hand-off known only by its hash: unreadable, old, it stays pending', async () => {
    const storage = memoryStorage()
    const id = recorded(storage, { state: 'pending', txHash: TX, chainId: 137 })
    // Long after it could have expired, with no receipt yet and no checkout: the page watches it until
    // its own watch ends, and it is still pending.
    const t = setup({ storage })
    await t.shop.request('shop.capabilities', undefined)
    await new Promise((r) => setTimeout(r, 300))
    expect(await t.shop.request('shop.status', { operation: id })).toMatchObject({ state: 'pending' })
    expect(t.events.some((e) => e.operation === id && (e.state === 'failed' || e.state === 'cancelled'))).toBe(false)
    // And the same item stays protected from a second purchase.
    expect((await t.buy()).operation).toBe(id)
  })

  it('skips a stored record that isn’t the shape this page writes, or another buyer’s', async () => {
    for (const record of [
      { state: 'pending', txHash: 'not a hash', chainId: 137 },
      { state: 'done', txHash: TX, chainId: 137 },
      { state: 'pending', txHash: TX, chainId: 137, buyer: OTHER },
      { state: 'pending', txHash: TX, chainId: 137, progress: { status: 'submitted' } },
      { state: 'pending', txHash: TX, chainId: 137, createdAt: 'yesterday' }
    ]) {
      const storage = memoryStorage()
      const id = recorded(storage, record)
      const t = setup({ storage })
      expect(await t.shop.request('shop.status', { operation: id })).toBeNull()
      expect(t.watch).not.toHaveBeenCalled()
    }
  })

  it('reads a record again once it holds its lock: the tab that had it may have ended it meanwhile', async () => {
    const storage = memoryStorage()
    const id = recorded(storage, { state: 'pending', txHash: TX, chainId: 137 })
    const release = vi.fn()
    const t = setup({
      storage,
      // The tab that held it settles it and lets go just before this one takes the lock.
      claim: async () => {
        recorded(storage, { state: 'succeeded', txHash: TX, chainId: 137, updatedAt: 2 })
        return release
      }
    })
    expect(await t.shop.request('shop.status', { operation: id })).toMatchObject({ state: 'succeeded' })
    expect(release).toHaveBeenCalled()
    await new Promise((r) => setTimeout(r, 50))
    expect(t.events.some((e) => e.operation === id)).toBe(false)
  })

  it('never rebuilds, under its lock, a record that went from storage meanwhile', async () => {
    const storage = memoryStorage()
    const id = recorded(storage, { state: 'pending', txHash: TX, chainId: 137 })
    const release = vi.fn()
    const t = setup({
      storage,
      claim: async () => {
        storage.removeItem(`dcl-inworld-shop:record:v1:${BUYER}:${id}`)
        return release
      }
    })
    expect(await t.shop.request('shop.status', { operation: id })).toBeNull()
    expect(release).toHaveBeenCalled()
    await new Promise((r) => setTimeout(r, 50))
    expect(t.events.some((e) => e.operation === id)).toBe(false)
    expect(storage.getItem(`dcl-inworld-shop:record:v1:${BUYER}:${id}`)).toBeNull()
  })

  it('settles a hand-off known only by its hash from its receipt on its chain', async () => {
    for (const [status, state] of [['success', 'succeeded'], ['reverted', 'failed']] as const) {
      const storage = memoryStorage()
      const id = recorded(storage, { state: 'pending', txHash: TX, chainId: 137 })
      const t = setup({ storage, receipts: { [TX]: status } })
      await t.shop.request('shop.capabilities', undefined)
      await vi.waitFor(() => expect(t.events.find((e) => e.operation === id)?.state).toBe(state))
    }
  })

  it('ends a restored attempt with no checkpoint and no hand-off: nothing could have left the browser', async () => {
    const storage = memoryStorage()
    const id = recorded(storage, { state: 'processing', chainId: 137 })
    const t = setup({ storage })
    await t.shop.request('shop.capabilities', undefined)
    await vi.waitFor(() => expect(t.events.find((e) => e.operation === id)?.state).toBe('cancelled'))
  })

  it('tries again as a fresh purchase once the storage works again', async () => {
    const storage = memoryStorage()
    const write = storage.setItem.bind(storage)
    let refuse = true
    storage.setItem = (key, value) => {
      if (refuse && key.includes(':record:') && value.includes('"processing"')) throw new DOMException('full', 'QuotaExceededError')
      write(key, value)
    }
    const t = setup({ storage })
    const first = await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    const failed = await t.until('failed')
    expect(t.run).not.toHaveBeenCalled()
    refuse = false
    failed.handlers.onRetry?.()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    await t.until('success')
    expect(t.run).toHaveBeenCalledTimes(1)
    expect(t.events.find((e) => e.operation === first.operation && e.state === 'cancelled')).toBeTruthy()
    expect(t.events.at(-1)?.operation).not.toBe(first.operation)
  })

  it('shows the result in the pending dialog when it is still up: a late success', async () => {
    let settledNow = false
    const t = setup({
      run: async (deps, group) => {
        handedOff(deps, group)
        return { status: 'pending', txHash: TX }
      },
      watch: async () => (settledNow ? { status: 'confirmed', txHash: TX } : { status: 'pending', txHash: TX })
    })
    await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    await t.until('pending')
    settledNow = true
    const { view } = await t.until('success')
    expect(view).toMatchObject({ items: [{ name: 'Golden Armour' }] })
  })

  it('shows the result in the pending dialog when it is still up: a late revert', async () => {
    const t = setup({
      run: async (deps, group) => {
        handedOff(deps, group)
        return { status: 'pending', txHash: TX }
      },
      watch: async () => ({ status: 'failed', failure: { kind: 'reverted', message: 'reverted' }, released: [] })
    })
    await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    const { view } = await t.until('failed')
    expect(view).toMatchObject({ retry: false })
    expect(view.step === 'failed' && view.message).toMatch(/failed on-chain/)
  })

  it('leaves another purchase’s dialog alone when a closed one settles', async () => {
    let settledNow = false
    const t = setup({
      tick: 0,
      run: async (deps, group) => {
        handedOff(deps, group)
        return { status: 'pending', txHash: TX }
      },
      watch: async () => (settledNow ? { status: 'confirmed', txHash: TX } : { status: 'pending', txHash: TX })
    })
    const first = await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    ;(await t.until('pending')).handlers.onClose()
    await t.buy('8')
    await t.until('confirm')
    settledNow = true
    await vi.waitFor(() => expect(t.events.find((e) => e.operation === first.operation && e.state === 'succeeded')).toBeTruthy())
    expect(t.last().view.step).toBe('confirm')
  })

  it('cancels, after a reload, a purchase that reserved a credit and handed nothing off', async () => {
    const storage = memoryStorage()
    const before = setup({
      storage,
      run: (deps, group) => {
        deps.store.dispatch(group.keyId, { type: 'authorize' })
        deps.store.dispatch(group.keyId, { type: 'authorized', creditIds: ['0x' + 'a1'.repeat(32)] })
        return new Promise<never>(() => {})
      }
    })
    const op = await before.buy()
    ;(await before.until('confirm')).handlers.onConfirm?.()
    await before.until('processing')
    const after = setup({ storage })
    await after.shop.request('shop.capabilities', undefined)
    await vi.waitFor(() => expect(after.events.find((e) => e.operation === op.operation)?.state).toBe('cancelled'))
    expect(after.watch).not.toHaveBeenCalled()
  })

  it('closes, after a reload, a dialog that was still reviewing', async () => {
    const storage = memoryStorage()
    const before = setup({ storage })
    const op = await before.buy()
    await before.until('confirm')
    const after = setup({ storage })
    expect(await after.shop.request('shop.status', { operation: op.operation })).toMatchObject({ state: 'cancelled' })
  })

  it('offers to try again after a declined signature, reading the terms anew', async () => {
    const t = setup({ run: async () => ({ status: 'failed', failure: { kind: 'rejected', message: 'You declined the signature.' }, released: [] }) })
    await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    const failed = await t.until('failed')
    expect(failed.view).toMatchObject({ message: 'You declined the signature.', retry: true })
    failed.handlers.onRetry?.()
    await t.until('confirm')
    expect(t.run).toHaveBeenCalledTimes(1)
  })

  it('turns a top-up refusal into Insufficient Funds', async () => {
    const t = setup({ run: async () => ({ status: 'failed', failure: { kind: 'refused', message: 'short' }, topUp: { requiredCents: 230, balanceCents: 50 }, released: [] }) })
    await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    expect((await t.until('insufficient')).view).toMatchObject({ balance: 5 })
  })

  it('says an item that is no longer listed isn’t on sale', async () => {
    const t = setup({ listing: null })
    await t.buy()
    const { view } = await t.until('failed')
    expect(view.step === 'failed' && view.message).toMatch(/isn't on sale/)
  })

  it('leaves alone a purchase another live tab holds, and its record with it', async () => {
    const storage = memoryStorage()
    const browser = locks()
    let answer: (r: GroupResult) => void = () => {}
    const first = setup({
      storage,
      claim: browser.tab('first'),
      // Mid-purchase: a credit reserved, the wallet still answering.
      run: (deps, group) => {
        deps.store.dispatch(group.keyId, { type: 'authorize' })
        deps.store.dispatch(group.keyId, { type: 'authorized', creditIds: ['0x' + 'a1'.repeat(32)] })
        return new Promise((resolve) => (answer = resolve))
      }
    })
    const op = await first.buy()
    ;(await first.until('confirm')).handlers.onConfirm?.()
    await first.until('processing')
    expect(browser.holds(op.operation)).toBe('first')
    // A second tab over the same storage sees it as it stands, and neither ends nor follows it.
    const second = setup({ storage, claim: browser.tab('second') })
    expect(await second.shop.request('shop.status', { operation: op.operation })).toMatchObject({ state: 'processing' })
    await new Promise((r) => setTimeout(r, 50))
    expect(second.events.some((e) => e.operation === op.operation)).toBe(false)
    expect(second.watch).not.toHaveBeenCalled()
    // Buying the same item there shows the purchase under way, not a second one.
    expect((await second.buy()).operation).toBe(op.operation)
    expect(second.last().view.step).toBe('pending')
    expect(second.run).not.toHaveBeenCalled()
    // The first tab finishes its own purchase, and lets it go.
    answer({ status: 'confirmed', txHash: TX })
    await first.until('success')
    expect(first.states()).toEqual(['reviewing', 'processing', 'succeeded'])
    expect(browser.holds(op.operation)).toBeUndefined()
    // The second tab, queued for its lock, learns how it ended unasked: its app hears of it, and its
    // dialog says so.
    await vi.waitFor(() => expect(second.events.at(-1)).toMatchObject({ operation: op.operation, state: 'succeeded' }))
    expect(second.last().view.step).toBe('success')
    expect(second.run).not.toHaveBeenCalled()
    expect(second.watch).not.toHaveBeenCalled()
    expect(browser.holds(op.operation)).toBeUndefined()
  })

  it('takes over a hand-off once the tab that held it is gone', async () => {
    const storage = memoryStorage()
    const browser = locks()
    const first = setup({
      storage,
      claim: browser.tab('first'),
      run: async (deps, group) => {
        handedOff(deps, group)
        return { status: 'pending', txHash: TX }
      }
    })
    const op = await first.buy()
    ;(await first.until('confirm')).handlers.onConfirm?.()
    await first.until('pending')
    const second = setup({ storage, claim: browser.tab('second'), watch: async () => ({ status: 'confirmed', txHash: TX }) })
    await second.shop.request('shop.capabilities', undefined)
    await new Promise((r) => setTimeout(r, 50))
    expect(second.watch).not.toHaveBeenCalled()
    // The first tab closes: its locks go with it, and the second, queued for it, follows the purchase to
    // its end unasked.
    browser.close('first')
    await vi.waitFor(() => expect(second.events.find((e) => e.operation === op.operation && e.state === 'succeeded')).toBeTruthy())
    expect(second.run).not.toHaveBeenCalled()
    expect(browser.holds(op.operation)).toBeUndefined()
  })

  it('ends a hand-off whose hash never mined once its credit expired unused, and frees the item', async () => {
    const t = setup({
      run: async (deps, group) => {
        handedOff(deps, group)
        return { status: 'pending', txHash: TX }
      },
      creditRecord: async () => 'expired',
      tick: 5 * 60_000
    })
    const first = await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    await vi.waitFor(() => expect(t.events.find((e) => e.operation === first.operation && e.state === 'failed')).toBeTruthy())
    // Not on the first expired read: read again past the index's lag (15 minutes, here 5 a wait).
    expect(t.creditRecord).toHaveBeenCalledTimes(4)
    expect(t.creditRecord.mock.calls[0][1]).toEqual(['0x' + 'a1'.repeat(32)])
    const { view, handlers } = await t.until('failed')
    expect(view).toMatchObject({ retry: false, message: expect.stringMatching(/never went through/) })
    handlers.onClose()
    // The item can be bought again.
    const again = await t.buy()
    expect(again.operation).not.toBe(first.operation)
    await t.until('confirm')
  })

  it('settles a hand-off whose hash has no receipt when its credit was spent', async () => {
    const t = setup({
      run: async (deps, group) => {
        handedOff(deps, group)
        return { status: 'pending', txHash: TX }
      },
      creditRecord: async () => 'settled'
    })
    const op = await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    await vi.waitFor(() => expect(t.events.find((e) => e.operation === op.operation && e.state === 'succeeded')).toBeTruthy())
    expect(t.last().view.step).toBe('success')
  })

  it('names the creator from their profile, not the app, and by address without one', async () => {
    const named = setup({ creatorName: async () => 'Marlene' })
    await named.buy()
    expect((await named.until('confirm')).view).toMatchObject({ items: [{ creator: 'Marlene' }] })
    const unnamed = setup({ creatorName: async () => null })
    await unnamed.buy()
    const { view } = await unnamed.until('confirm')
    const creator = String(BOOK.creator)
    expect(view).toMatchObject({ items: [{ creator: `${creator.slice(0, 6)}…${creator.slice(-4)}` }] })
  })

  it('answers shop.status with the signed-in buyer’s operations only', async () => {
    let account = BUYER
    const t = setup({ login: () => login(account) })
    const op = await t.buy()
    ;(await t.until('confirm')).handlers.onClose()
    account = OTHER
    expect(await t.shop.request('shop.status', { operation: op.operation })).toBeNull()
    expect(await t.shop.request('shop.status', {})).toEqual([])
  })

  it('signs the terms’ requests with the login that read them and no later one', async () => {
    let held = login()
    const t = setup({ login: () => held })
    await t.buy()
    await t.until('confirm')
    const quote = t.signers[0]
    expect(quote()).not.toBeNull()
    held = { ...held, identity: { ...held.identity, expiration: new Date(Date.now() + 7_200_000).toISOString() } }
    expect(quote()).toBeNull()
  })

  it('lets go of an operation’s lock once it is cancelled, or once it failed', async () => {
    const browser = locks()
    const cancelled = setup({ claim: browser.tab('first') })
    const a = await cancelled.buy()
    expect(browser.holds(a.operation)).toBe('first')
    ;(await cancelled.until('confirm')).handlers.onClose()
    expect(browser.holds(a.operation)).toBeUndefined()
    const failing = setup({ claim: browser.tab('second'), run: async () => ({ status: 'failed', failure: { kind: 'error', message: 'The wallet could not sign.' }, released: [] }) })
    const b = await failing.buy()
    ;(await failing.until('confirm')).handlers.onConfirm?.()
    ;(await failing.until('failed')).handlers.onClose()
    expect(failing.states().at(-1)).toBe('failed')
    expect(browser.holds(b.operation)).toBeUndefined()
  })

  it('ends what a closed tab left open: a dialog it was reviewing, a credit it reserved and never sent', async () => {
    const storage = memoryStorage()
    const browser = locks()
    const CREDIT = '0x' + 'a1'.repeat(32)
    const reviewing = setup({ storage, claim: browser.tab('reviewing') })
    const a = await reviewing.buy()
    await reviewing.until('confirm')
    const reserving = setup({
      storage,
      claim: browser.tab('reserving'),
      run: (deps, group) => {
        deps.store.dispatch(group.keyId, { type: 'authorize' })
        deps.store.dispatch(group.keyId, { type: 'authorized', creditIds: [CREDIT] })
        return new Promise<never>(() => {})
      }
    })
    const b = await reserving.buy('8')
    ;(await reserving.until('confirm')).handlers.onConfirm?.()
    await reserving.until('processing')
    const other = setup({ storage, claim: browser.tab('other') })
    await other.shop.request('shop.capabilities', undefined)
    await new Promise((r) => setTimeout(r, 20))
    expect(other.events).toEqual([])
    // Both tabs close: the third, queued for their locks, ends both, and gives the reserved credit back.
    browser.close('reviewing')
    browser.close('reserving')
    await vi.waitFor(() => expect(other.events.filter((e) => e.state === 'cancelled').map((e) => e.operation).sort()).toEqual([a.operation, b.operation].sort()))
    expect(other.cancels).toEqual([[CREDIT]])
    expect(other.watch).not.toHaveBeenCalled()
  })

  it('takes over another tab’s purchase only for the buyer it was made for', async () => {
    const storage = memoryStorage()
    const browser = locks()
    const first = setup({
      storage,
      claim: browser.tab('first'),
      run: async (deps, group) => {
        handedOff(deps, group)
        return { status: 'pending', txHash: TX }
      }
    })
    const op = await first.buy()
    ;(await first.until('confirm')).handlers.onConfirm?.()
    await first.until('pending')
    let account = BUYER
    const second = setup({ storage, claim: browser.tab('second'), login: () => login(account), watch: async () => ({ status: 'confirmed', txHash: TX }) })
    await second.shop.request('shop.capabilities', undefined)
    // Another account signs in here before the first tab goes: its lock comes and is let go untouched.
    account = OTHER
    browser.close('first')
    await new Promise((r) => setTimeout(r, 20))
    expect(browser.holds(op.operation)).toBeUndefined()
    expect(second.watch).not.toHaveBeenCalled()
    // Back as the buyer, the next request takes it over.
    account = BUYER
    await second.shop.request('shop.status', {})
    await vi.waitFor(() => expect(second.events.find((e) => e.operation === op.operation && e.state === 'succeeded')).toBeTruthy())
  })

  it('keeps reading an unsettled purchase past its first half hour, every five minutes', async () => {
    const t = setup({
      run: async (deps, group) => {
        handedOff(deps, group)
        return { status: 'pending', txHash: TX }
      }
    })
    await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    ;(await t.until('pending')).handlers.onClose()
    await vi.waitFor(() => expect(t.sleeps.filter((ms) => ms === 5 * 60_000).length).toBeGreaterThanOrEqual(3))
    const reads = t.watch.mock.calls.length
    await vi.waitFor(() => expect(t.watch.mock.calls.length).toBeGreaterThan(reads))
    expect(t.states().at(-1)).toBe('pending')
  })

  it('reads an expired credit twice, the window starting again after it reads open', async () => {
    const answers: Array<'expired' | 'open'> = ['expired', 'open']
    const t = setup({
      run: async (deps, group) => {
        handedOff(deps, group)
        return { status: 'pending', txHash: TX }
      },
      creditRecord: async () => answers.shift() ?? 'expired',
      tick: 5 * 60_000
    })
    const op = await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    await vi.waitFor(() => expect(t.events.find((e) => e.operation === op.operation && e.state === 'failed')).toBeTruthy())
    // Expired at 0, open at 5, then expired from 10: it fails at 25, fifteen minutes after that.
    expect(t.creditRecord).toHaveBeenCalledTimes(6)
  })

  it('runs no purchase it can’t hold to this tab', async () => {
    const t = setup({ claim: async () => null })
    await t.buy()
    const { view } = await t.until('failed')
    expect(view).toMatchObject({ retry: false, message: expect.stringMatching(/can't keep the purchase to this tab/) })
    expect(t.run).not.toHaveBeenCalled()
    expect(t.provider.requests).toEqual([])
  })

  it('runs one dialog at a time', async () => {
    const t = setup()
    await t.buy()
    await expect(t.buy('8')).rejects.toThrow(/already open/)
  })

  it('refuses a request that names more than an item', async () => {
    const t = setup()
    await expect(t.shop.request('shop.buy', { v: 1, items: [{ contract: CONTRACT, itemId: '7', quantity: 1 }], method: 'credits', buyer: BUYER })).rejects.toThrow(/unexpected/)
    await expect(t.shop.request('shop.buy', { v: 1, items: [{ contract: CONTRACT, itemId: '7', quantity: 1 }], method: 'mana' })).rejects.toThrow(/method/)
  })

  it('tells the HUD what the session can pay with', async () => {
    expect(await setup().shop.request('shop.capabilities', undefined)).toEqual({ v: 1, buyer: BUYER, wallet: 'managed', credits: true })
    expect(await setup({ login: () => null }).shop.request('shop.capabilities', undefined)).toMatchObject({ buyer: null, credits: false, reason: 'signedOut' })
    expect(await setup({ login: () => login(BUYER, { guest: true }) }).shop.request('shop.capabilities', undefined)).toMatchObject({ credits: false, reason: 'guest' })
    expect(await setup({ wallet: null }).shop.request('shop.capabilities', undefined)).toMatchObject({ credits: false, reason: 'noWallet' })
  })

  it('reads the balance for the login, and none signed out', async () => {
    expect(await setup({ balanceCents: 1234 }).shop.request('shop.balance', undefined)).toEqual({ v: 1, credits: 123 })
    expect(await setup({ login: () => null }).shop.request('shop.balance', undefined)).toBeNull()
  })
})

describe('an operation’s lock', () => {
  /** Web Locks as the browser grants them: one holder per name, released when its callback's promise settles. */
  function webLocks(): LockManager {
    const held = new Set<string>()
    const queues = new Map<string, Promise<void>>()
    const request = async (name: string, options: LockOptions, granted: LockGrantedCallback<unknown>): Promise<unknown> => {
      if (options.ifAvailable && held.has(name)) return granted(null)
      // Without ifAvailable, after whoever holds it or is queued before.
      const before = queues.get(name) ?? Promise.resolve()
      let done = (): void => {}
      const mine = new Promise<void>((resolve) => (done = resolve))
      queues.set(name, before.then(() => mine))
      await before
      held.add(name)
      try {
        return await granted({ name, mode: 'exclusive' } as Lock)
      } finally {
        held.delete(name)
        done()
      }
    }
    return { request, query: async () => ({ held: [], pending: [] }) } as unknown as LockManager
  }

  it('is held by one page until it lets go', async () => {
    const locks = webLocks()
    const release = await claimLock(locks, 'op-1')
    expect(release).toBeTypeOf('function')
    expect(await claimLock(locks, 'op-1')).toBeNull()
    expect(await claimLock(locks, 'op-2')).toBeTypeOf('function')
    release!()
    await new Promise((r) => setTimeout(r, 0))
    expect(await claimLock(locks, 'op-1')).toBeTypeOf('function')
  })

  it('is queued for, when asked, until its holder lets go', async () => {
    const locks = webLocks()
    const release = await claimLock(locks, 'op-1')
    let granted: (() => void) | null = null
    void claimLock(locks, 'op-1', true).then((r) => (granted = r))
    await new Promise((r) => setTimeout(r, 0))
    expect(granted).toBeNull()
    release!()
    await vi.waitFor(() => expect(granted).toBeTypeOf('function'))
  })

  it('is never taken without Web Locks: nothing proves this page is the only one', async () => {
    expect(await claimLock(undefined, 'op-1')).toBeNull()
  })
})

describe('the purchase ledger over one storage, from two tabs', () => {
  const record = (id: string, state: OperationRecord['state'], at: number): OperationRecord => ({
    id,
    buyer: BUYER,
    items: [{ contract: CONTRACT, itemId: '7', quantity: 1 }],
    state,
    createdAt: at,
    updatedAt: at
  })

  it('never loses one tab’s operation to the other’s write', () => {
    const storage = memoryStorage()
    const first = createLedger(() => storage)
    const second = createLedger(() => storage)
    const a = '0f0e0d0c-0000-4000-8000-00000000000a'
    const b = '0f0e0d0c-0000-4000-8000-00000000000b'
    expect(first.save(record(a, 'processing', 1))).toBe(true)
    // Each tab reads what is there, then both write their own, interleaved.
    expect(first.load(BUYER).map((r) => r.id)).toEqual([a])
    expect(second.save(record(b, 'processing', 2))).toBe(true)
    expect(first.save({ ...record(a, 'pending', 1), updatedAt: 3 })).toBe(true)
    expect(second.save({ ...record(b, 'pending', 2), updatedAt: 4 })).toBe(true)
    for (const ledger of [first, second]) expect(ledger.load(BUYER).map((r) => [r.id, r.state])).toEqual([[a, 'pending'], [b, 'pending']])
  })

  it('keeps every unsettled operation, dropping only the oldest settled ones past its bound', () => {
    const storage = memoryStorage()
    const ledger = createLedger(() => storage)
    const id = (n: number): string => `0f0e0d0c-0000-4000-8000-${String(n).padStart(12, '0')}`
    for (let n = 1; n <= 25; n++) ledger.save(record(id(n), n <= 3 ? 'pending' : 'succeeded', n))
    const kept = ledger.load(BUYER).map((r) => r.id)
    expect(kept).toHaveLength(20)
    expect(kept.slice(0, 3)).toEqual([id(1), id(2), id(3)])
    expect(kept).not.toContain(id(4))
    expect(kept).toContain(id(25))
  })
})

describe('a purchase of several items (the cart)', () => {
  const listing = (itemId: string, name: string, extra: Partial<CatalogItem> = {}): CatalogItem => ({ ...LISTING, id: `listing-${itemId}`, itemId, name, ...extra }) as CatalogItem
  const ITEMS = { '7': listing('7', 'Golden Armour'), '8': listing('8', 'Silver Boots'), '9': listing('9', 'Bronze Hat') }
  /** The store's mint of each line's own item. */
  const mints: ShellShopServices['offers'] = () => async (queries, session) =>
    Object.fromEntries(queries.map((q) => [q.key, storeMint(session, 'wearable', BigInt(q.asset.token.kind === 'item' ? q.asset.token.itemId : 0))]))
  /** Item 7 as its creator's listing on the marketplace; the rest minted by the store. */
  const mixed: ShellShopServices['offers'] = () => async (queries, session) =>
    Object.fromEntries(
      queries.map((q) => {
        const itemId = q.asset.token.kind === 'item' ? q.asset.token.itemId : '0'
        return [q.key, itemId === String(ITEM) ? creatorOrder(session) : storeMint(session, 'wearable', BigInt(itemId))]
      })
    )
  const trades: ShellShopServices['resolvers'] = () => ({ trade: async () => creatorTrade(), store: async () => ({ priceWei: '2300000000000000000', available: 10 }) })

  it('confirms every unit, one row each, and buys them in one transaction when they settle together', async () => {
    const t = setup({ items: ITEMS, offers: mints })
    const op = await t.checkout('7', '8x2')
    expect(op).toMatchObject({ cart: true, items: [{ itemId: '7', quantity: 1 }, { itemId: '8', quantity: 2 }] })
    expect(t.shown[0].view).toEqual({ step: 'loading', count: 3 })
    const { view, handlers } = await t.until('confirm')
    expect(view.step === 'confirm' && view.items.map((i) => i.name)).toEqual(['Golden Armour', 'Silver Boots', 'Silver Boots'])
    handlers.onConfirm?.()
    const done = await t.until('success')
    expect(done.view.step === 'success' && done.view.items).toHaveLength(3)
    // One group, one transaction: the guard bound to every unit of it.
    expect(t.run).toHaveBeenCalledTimes(1)
    const units = t.guard.mock.calls[0][1].units
    expect(units).toEqual([
      { contract: CONTRACT, itemId: '7' },
      { contract: CONTRACT, itemId: '8' },
      { contract: CONTRACT, itemId: '8' }
    ])
    expect(t.events.at(-1)).toMatchObject({ operation: op.operation, state: 'succeeded', cart: true, items: [{ itemId: '7' }, { itemId: '8', quantity: 2 }] })
  })

  it('counts the units in the dialog’s title and asks for what is missing across them all', async () => {
    // Enough for one of them, not for the three.
    const t = setup({ items: ITEMS, offers: mints, balanceCents: 10 })
    await t.checkout('7', '8', '9')
    const { view } = await t.until('insufficient')
    expect(view).toMatchObject({ step: 'insufficient', balance: 1 })
    expect(view.step === 'insufficient' && view.need).toBe(view.step === 'insufficient' ? view.items.reduce((n, i) => n + i.credits, 0) - 1 : 0)
    expect(view.step === 'insufficient' && view.items).toHaveLength(3)
  })

  it('runs items that settle apart as operations of their own, one after the other, under the one confirmation', async () => {
    const order: string[] = []
    const t = setup({
      items: ITEMS,
      offers: mixed,
      resolvers: trades,
      run: async (_deps, group) => {
        order.push(group.key.call.kind)
        return { status: 'confirmed', txHash: TX }
      }
    })
    const first = await t.checkout('7', '8')
    ;(await t.until('confirm')).handlers.onConfirm?.()
    const done = await t.until('success')
    expect(done.view.step === 'success' && done.view.items.map((i) => i.name)).toEqual(['Golden Armour', 'Silver Boots'])
    expect(order).toEqual(['accept', 'storeBuy'])
    // Each transaction's guard is bound to its own units and its own venue.
    expect(t.guard.mock.calls.map(([, intent]) => [intent.sale.call, intent.units.map((u) => u.itemId)])).toEqual([
      ['accept', ['7']],
      ['storeBuy', ['8']]
    ])
    const settled = t.events.filter((e) => e.state === 'succeeded')
    expect(settled.map((e) => [e.operation === first.operation, e.items.map((i) => i.itemId)])).toEqual([
      [true, ['7']],
      [false, ['8']]
    ])
    expect(settled.every((e) => e.cart)).toBe(true)
  })

  it('stops at a group that fails after another went out: what went stands, and the rest stays in the cart', async () => {
    const t = setup({
      items: ITEMS,
      offers: mixed,
      resolvers: trades,
      run: async (_deps, group) =>
        group.key.call.kind === 'accept' ? { status: 'confirmed', txHash: TX } : { status: 'failed', failure: { kind: 'rejected', message: 'You declined the signature.' }, released: [] }
    })
    await t.checkout('7', '8', '9')
    ;(await t.until('confirm')).handlers.onConfirm?.()
    const { view, handlers } = await t.until('failed')
    expect(view).toMatchObject({ step: 'failed', retry: false })
    expect(view.step === 'failed' && view.message).toMatch(/^Part of this purchase went through.*You declined the signature\..*still in your cart/)
    expect(handlers.onRetry).toBeUndefined()
    handlers.onClose()
    expect(t.events.filter((e) => e.state === 'succeeded').map((e) => e.items.map((i) => i.itemId))).toEqual([['7']])
    expect(t.events.filter((e) => e.state === 'failed').map((e) => e.items.map((i) => i.itemId))).toEqual([['8', '9']])
  })

  it('offers TRY AGAIN for the whole cart when its first group handed nothing off', async () => {
    let calls = 0
    const t = setup({
      items: ITEMS,
      offers: mixed,
      resolvers: trades,
      run: async () => (++calls === 1 ? { status: 'failed', failure: { kind: 'rejected', message: 'You declined the signature.' }, released: [] } : { status: 'confirmed', txHash: TX })
    })
    await t.checkout('7', '8')
    ;(await t.until('confirm')).handlers.onConfirm?.()
    const failed = await t.until('failed')
    expect(failed.view).toMatchObject({ message: 'You declined the signature.', retry: true })
    failed.handlers.onRetry?.()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    const done = await t.until('success')
    expect(done.view.step === 'success' && done.view.items).toHaveLength(2)
  })

  it('follows a group still confirming and goes on with the next, then says the purchase is in progress', async () => {
    const t = setup({
      items: ITEMS,
      offers: mixed,
      resolvers: trades,
      run: async (deps, group) => {
        if (group.key.call.kind !== 'accept') return { status: 'confirmed', txHash: TX }
        handedOff(deps, group)
        return { status: 'pending', txHash: TX }
      }
    })
    await t.checkout('7', '8')
    ;(await t.until('confirm')).handlers.onConfirm?.()
    await t.until('pending')
    expect(t.run).toHaveBeenCalledTimes(2)
    await vi.waitFor(() => expect(t.watch).toHaveBeenCalled())
    expect(t.events.filter((e) => e.state === 'succeeded').map((e) => e.items.map((i) => i.itemId))).toEqual([['8']])
  })

  it('names the item that can’t be bought, and buys none of them', async () => {
    const t = setup({ items: { ...ITEMS, '8': listing('8', 'Silver Boots', { available: 1 }) }, offers: mints })
    await t.checkout('7', '8x2')
    const { view } = await t.until('failed')
    expect(view).toMatchObject({ message: 'Only 1 left of Silver Boots.' })
    expect(t.run).not.toHaveBeenCalled()
  })

  it('shows the purchase under way when the cart holds an item still being bought', async () => {
    const t = setup({
      items: ITEMS,
      offers: mints,
      run: async (deps, group) => {
        handedOff(deps, group)
        return { status: 'pending', txHash: TX }
      }
    })
    const first = await t.buy('8')
    ;(await t.until('confirm')).handlers.onConfirm?.()
    await t.until('pending')
    t.last().handlers.onClose()
    const again = await t.checkout('7', '8')
    expect(again.operation).toBe(first.operation)
    expect(t.last().view.step).toBe('pending')
  })
})

