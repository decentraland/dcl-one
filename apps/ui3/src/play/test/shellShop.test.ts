// The web shell's side of the in-world Shop purchase (shell/shop/purchase): what it shows the buyer, when
// it signs, for whom, what it tells the HUD, and how it follows a purchase that outlives its dialog. The
// Shop's reads, the executor and its watcher are fakes here (shellShopRails runs the real ones); the cart
// partition, review and the per-operation ledger are the real ones.
import { describe, expect, it, vi } from 'vitest'
import type { Eip1193Provider } from '../../data/auth/wallet'
import type { CartGroup } from '../../shop/cart/partition'
import { createFakeWallet, type FakeWallet } from '../../shop/checkout/fakes'
import type { CheckoutDeps, GroupResult } from '../../shop/checkout/purchase/executor'
import type { CatalogItem } from '../../shop/discovery/lib/catalogItem'
import type { UsdCents } from '../../shop/model/money'
import { BOOK, ITEM, storeMint } from '../../shop/payment/fixtures'
import type { StoredLogin } from '../features/auth/sso'
import { SHOP_EVENT, SHOP_PROTOCOL, type ShopOperation, type ShopOperationEvent } from '../lib/shopProtocol'
import { createShellShop, type ShellPurchaseHandlers, type ShellPurchaseView, type ShellShopServices, type ShellShopWallet } from '../shell/shop/purchase'
import type { PurchaseIntent } from '../shell/shop/walletGuard'

const BUYER = BOOK.buyer.toLowerCase()
const OTHER = '0x' + '9'.repeat(40)
const CONTRACT = BOOK.collection.toLowerCase()
const TRUSTED = { chainId: 137, address: '0x' + 'ce'.repeat(20), name: 'Decentraland Credits', version: '1.0.0' }

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
function handedOff(deps: CheckoutDeps, group: CartGroup, txHash = '0xabc'): void {
  deps.store.dispatch(group.keyId, { type: 'authorize' })
  deps.store.dispatch(group.keyId, { type: 'authorized', creditIds: ['0x' + 'a1'.repeat(32)] })
  deps.store.dispatch(group.keyId, { type: 'submit' })
  deps.store.dispatch(group.keyId, { type: 'hash', txHash })
  deps.store.dispatch(group.keyId, { type: 'observed' })
}

type Shown = { view: ShellPurchaseView; handlers: ShellPurchaseHandlers }
type RunFn = (deps: CheckoutDeps, group: CartGroup) => Promise<GroupResult>

function setup(
  opts: {
    balanceCents?: number
    wallet?: ShellShopWallet | null
    account?: string
    login?: () => StoredLogin | null
    run?: RunFn
    watch?: () => Promise<GroupResult>
    listing?: CatalogItem | null
    storage?: Storage
    manager?: typeof TRUSTED | null
  } = {}
) {
  const shown: Shown[] = []
  const events: ShopOperationEvent[] = []
  const provider: FakeWallet = createFakeWallet({ account: opts.account ?? BUYER })
  const wallet: ShellShopWallet | null = opts.wallet === undefined ? { kind: vi.fn(() => 'managed' as const), provider: vi.fn(() => provider) } : opts.wallet
  const run = vi.fn<RunFn>(opts.run ?? (async () => ({ status: 'confirmed', txHash: '0xabc' })))
  const watch = vi.fn(opts.watch ?? (async (): Promise<GroupResult> => ({ status: 'pending', txHash: '0xabc' })))
  const guard = vi.fn((p: Eip1193Provider, _intent: PurchaseIntent) => p)
  const cents = (opts.balanceCents ?? 7000) as UsdCents
  let clock = 1_000_000
  let ids = 0
  const services: Partial<ShellShopServices> = {
    client: () => ({}) as never,
    fetchItem: async () => (opts.listing === undefined ? LISTING : opts.listing),
    offers: () => async (queries, session) => Object.fromEntries(queries.map((q) => [q.key, storeMint(session)])),
    balance: async () => ({ balanceCents: cents, credits: Math.floor(cents / 10) }),
    resolvers: () => ({ trade: async () => null, store: async () => ({ priceWei: '2300000000000000000', available: 10 }) }),
    reader: () => ({}) as never,
    primeFlags: async () => {},
    run: run as unknown as ShellShopServices['run'],
    watch,
    manager: () => (opts.manager === undefined ? TRUSTED : opts.manager),
    guard,
    storage: () => opts.storage ?? null,
    // The clock moves with each wait, and the wait yields, so a test can change what the watcher sees.
    sleep: async (ms) => {
      clock += ms
      await new Promise((resolve) => setTimeout(resolve, 2))
    },
    now: () => clock,
    newId: () => `0f0e0d0c-0000-4000-8000-${String(++ids).padStart(12, '0')}`,
    creditsUrl: () => 'https://decentraland.example/shop/credits'
  }
  const deps = {
    login: opts.login ?? (() => login()),
    wallet,
    ui: { show: vi.fn((view: ShellPurchaseView, handlers: ShellPurchaseHandlers) => void shown.push({ view, handlers })), hide: vi.fn() },
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
  const buy = (itemId = String(ITEM)) => shop.request('shop.buy', { v: SHOP_PROTOCOL, item: { contract: CONTRACT, itemId }, method: 'credits', creatorName: 'Aurora' }) as Promise<ShopOperation>
  const states = (): string[] => events.map((e) => e.state)
  return { shop, deps, shown, events, run, watch, guard, provider, wallet, last, until, buy, states }
}

describe('the shell purchase', () => {
  it('answers shop.buy at once with the operation, then shows the item, its price and the balance', async () => {
    const t = setup()
    const op = await t.buy()
    expect(op).toMatchObject({ state: 'reviewing', item: { contract: CONTRACT, itemId: '7' } })
    expect(t.shown[0].view.step).toBe('loading')
    const { view } = await t.until('confirm')
    expect(view).toMatchObject({ step: 'confirm', balance: 700, item: { name: 'Golden Armour', creator: 'Aurora', rarity: 'epic', category: 'upper_body' } })
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
    expect(intent).toMatchObject({ buyer: BUYER, chainId: 137, manager: TRUSTED, item: { contract: CONTRACT, itemId: '7' }, sale: { venue: BOOK.store.contract.toLowerCase(), call: 'storeBuy' } })
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
    expect(view.step === 'failed' && view.message).toMatch(/another account/)
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
        return { status: 'pending', txHash: '0xabc' }
      },
      watch: async () => (settled ? { status: 'confirmed', txHash: '0xabc' } : { status: 'pending', txHash: '0xabc' })
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
        return { status: 'pending', txHash: '0xabc' }
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
        return { status: 'pending', txHash: '0xabc' }
      }
    })
    const op = await before.buy()
    ;(await before.until('confirm')).handlers.onConfirm?.()
    await before.until('pending')
    // The page reloads: a new shell over the same storage.
    const after = setup({ storage, watch: async () => ({ status: 'confirmed', txHash: '0xabc' }) })
    expect(await after.shop.request('shop.status', { operation: op.operation })).toMatchObject({ operation: op.operation })
    await vi.waitFor(() => expect(after.events.find((e) => e.operation === op.operation && e.state === 'succeeded')).toBeTruthy())
    expect(after.watch).toHaveBeenCalled()
    expect(after.run).not.toHaveBeenCalled()
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

  it('runs one dialog at a time', async () => {
    const t = setup()
    await t.buy()
    await expect(t.buy('8')).rejects.toThrow(/already open/)
  })

  it('refuses a request that names more than an item', async () => {
    const t = setup()
    await expect(t.shop.request('shop.buy', { v: 1, item: { contract: CONTRACT, itemId: '7' }, method: 'credits', buyer: BUYER })).rejects.toThrow(/unexpected/)
    await expect(t.shop.request('shop.buy', { v: 1, item: { contract: CONTRACT, itemId: '7' }, method: 'mana' })).rejects.toThrow(/method/)
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
