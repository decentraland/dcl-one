// The web shell's side of the in-world Shop purchase (shell/shop/purchase): what it shows the buyer, when
// it signs, for whom, and what it tells the HUD. The Shop's reads, the executor and the wallet are fakes;
// the cart partition and review are the web Shop's own.
import { describe, expect, it, vi } from 'vitest'
import type { Eip1193Provider } from '../../data/auth/wallet'
import type { GroupResult } from '../../shop/checkout/purchase/executor'
import type { CatalogItem } from '../../shop/discovery/lib/catalogItem'
import type { UsdCents } from '../../shop/model/money'
import { BOOK, ITEM, storeMint } from '../../shop/payment/fixtures'
import type { StoredLogin } from '../features/auth/sso'
import { SHOP_EVENT, SHOP_PROTOCOL, type ShopOperationEvent } from '../lib/shopProtocol'
import { createShellShop, type ShellPurchaseHandlers, type ShellPurchaseView, type ShellShopServices, type ShellShopWallet } from '../shell/shop/purchase'

const BUYER = BOOK.buyer.toLowerCase()
const CONTRACT = BOOK.collection.toLowerCase()

function login(account = BUYER, extra: Partial<StoredLogin> = {}): StoredLogin {
  return {
    address: account,
    identity: {
      ephemeralIdentity: { address: '0x' + '1'.repeat(40), publicKey: '0x04', privateKey: '0x' + 'ab'.repeat(32) },
      expiration: new Date(Date.now() + 3_600_000).toISOString(),
      authChain: [
        { type: 'SIGNER', payload: account, signature: '' },
        { type: 'ECDSA_EPHEMERAL', payload: 'Decentraland Login', signature: '0x' }
      ]
    },
    ...extra
  }
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

const PRICE_CENTS = 230

type Shown = { view: ShellPurchaseView; handlers: ShellPurchaseHandlers }

function setup(opts: { balanceCents?: number; wallet?: ShellShopWallet | null; login?: () => StoredLogin | null; run?: () => Promise<GroupResult>; listing?: CatalogItem | null } = {}) {
  const shown: Shown[] = []
  const events: ShopOperationEvent[] = []
  const provider: Eip1193Provider = { request: vi.fn(async () => null) }
  const wallet: ShellShopWallet | null =
    opts.wallet === undefined ? { kind: vi.fn(() => 'managed' as const), provider: vi.fn(() => provider) } : opts.wallet
  const run = vi.fn(opts.run ?? (async (): Promise<GroupResult> => ({ status: 'confirmed', txHash: '0xabc' })))
  const guard = vi.fn((p: Eip1193Provider) => p)
  const cents = (opts.balanceCents ?? 7000) as UsdCents
  const services: Partial<ShellShopServices> = {
    client: () => ({}) as never,
    fetchItem: async () => (opts.listing === undefined ? LISTING : opts.listing),
    offers: () => async (queries, session) => Object.fromEntries(queries.map((q) => [q.key, storeMint(session)])),
    balance: async () => ({ balanceCents: cents, credits: Math.floor(cents / 10) }),
    resolvers: () => ({ trade: async () => null, store: async () => ({ priceWei: '2300000000000000000', available: 10 }) }),
    reader: () => ({}) as never,
    primeFlags: async () => {},
    run,
    guard,
    newId: () => '0f0e0d0c-0000-4000-8000-000000000001',
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
  const buy = () => shop.request('shop.buy', { v: SHOP_PROTOCOL, item: { contract: CONTRACT, itemId: String(ITEM) }, method: 'credits', creatorName: 'Aurora' })
  return { shop, deps, shown, events, run, guard, provider, wallet, last, until, buy }
}

describe('the shell purchase', () => {
  it('answers shop.buy at once with the operation, then shows the item, its price and the balance', async () => {
    const t = setup()
    const op = await t.buy()
    expect(op).toMatchObject({ operation: '0f0e0d0c-0000-4000-8000-000000000001', state: 'reviewing', item: { contract: CONTRACT, itemId: '7' } })
    expect(t.shown[0].view.step).toBe('loading')
    const { view } = await t.until('confirm')
    expect(view).toMatchObject({ step: 'confirm', balance: 700, item: { name: 'Golden Armour', creator: 'Aurora', rarity: 'epic', category: 'upper_body' } })
    expect(view.step === 'confirm' && view.item.credits).toBeGreaterThan(0)
    expect(t.run).not.toHaveBeenCalled()
  })

  it('signs nothing until BUY ITEM, then pays for the login’s own account through the guarded wallet', async () => {
    const t = setup()
    await t.buy()
    const confirm = await t.until('confirm')
    confirm.handlers.onConfirm?.()
    await t.until('success')
    expect(t.run).toHaveBeenCalledTimes(1)
    const [deps] = t.run.mock.calls[0] as unknown as [{ ctx: { buyer: string; walletKind: string; provider: unknown } }]
    expect(deps.ctx.buyer).toBe(BUYER)
    expect(deps.ctx.walletKind).toBe('managed')
    expect(t.guard).toHaveBeenCalledWith(t.provider, BUYER)
    expect(t.events.map((e) => e.state)).toEqual(['reviewing', 'processing', 'succeeded'])
    expect(t.events.every((e) => e.type === SHOP_EVENT && e.v === SHOP_PROTOCOL)).toBe(true)
  })

  it('sends the HUD to the Backpack from the success dialog', async () => {
    const t = setup()
    await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    const success = await t.until('success')
    success.handlers.onBackpack?.()
    expect(t.deps.ui.hide).toHaveBeenCalled()
    expect(t.events[t.events.length - 1]).toMatchObject({ state: 'succeeded', next: 'backpack' })
  })

  it('shows Insufficient Funds when the balance is short, and BUY CREDITS opens the credit packs', async () => {
    const t = setup({ balanceCents: 100 })
    await t.buy()
    const { view, handlers } = await t.until('insufficient')
    expect(view).toMatchObject({ step: 'insufficient', balance: 10 })
    expect(view.step === 'insufficient' && view.need).toBe(view.step === 'insufficient' ? view.item.credits - 10 : 0)
    handlers.onBuyCredits?.()
    expect(t.deps.openExternal).toHaveBeenCalledWith('https://decentraland.example/shop/credits')
    expect(t.events[t.events.length - 1].state).toBe('cancelled')
    expect(t.run).not.toHaveBeenCalled()
  })

  it('cancels without signing', async () => {
    const t = setup()
    await t.buy()
    ;(await t.until('confirm')).handlers.onClose()
    expect(t.deps.ui.hide).toHaveBeenCalled()
    expect(t.events.map((e) => e.state)).toEqual(['reviewing', 'cancelled'])
    expect(t.run).not.toHaveBeenCalled()
  })

  it('asks a signed-out buyer, or a guest, to sign in', async () => {
    for (const who of [() => null, () => login(BUYER, { guest: true })]) {
      const t = setup({ login: who })
      await t.buy()
      const { handlers } = await t.until('signIn')
      handlers.onSignIn?.()
      expect(t.deps.signIn).toHaveBeenCalled()
      expect(t.events[t.events.length - 1].state).toBe('cancelled')
    }
  })

  it('says so when no wallet here can sign for the login', async () => {
    const t = setup({ wallet: { kind: () => null, provider: () => null } })
    await t.buy()
    const { view } = await t.until('failed')
    expect(view).toMatchObject({ step: 'failed', retry: false })
  })

  it('refuses to pay when the login changed between the quote and BUY ITEM', async () => {
    let account = BUYER
    const t = setup({ login: () => login(account) })
    await t.buy()
    const confirm = await t.until('confirm')
    account = '0x' + '9'.repeat(40)
    confirm.handlers.onConfirm?.()
    const { view } = await t.until('failed')
    expect(view.step === 'failed' && view.message).toMatch(/another account/)
    expect(t.run).not.toHaveBeenCalled()
  })

  it('leaves a handed-off purchase pending, never failed', async () => {
    const t = setup({ run: async () => ({ status: 'pending', txHash: null }) })
    await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    await t.until('pending')
    expect(t.events[t.events.length - 1].state).toBe('pending')
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
    const t = setup({ run: async () => ({ status: 'failed', failure: { kind: 'refused', message: 'short' }, topUp: { requiredCents: PRICE_CENTS, balanceCents: 50 }, released: [] }) })
    await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    const { view } = await t.until('insufficient')
    expect(view).toMatchObject({ balance: 5 })
  })

  it('says an item that is no longer listed isn’t on sale', async () => {
    const t = setup({ listing: null })
    await t.buy()
    const { view } = await t.until('failed')
    expect(view.step === 'failed' && view.message).toMatch(/isn't on sale/)
  })

  it('lets the next purchase open once one is left pending', async () => {
    const t = setup({ run: async () => ({ status: 'pending', txHash: '0xabc' }) })
    await t.buy()
    ;(await t.until('confirm')).handlers.onConfirm?.()
    ;(await t.until('pending')).handlers.onClose()
    await expect(t.buy()).resolves.toMatchObject({ state: 'reviewing' })
  })

  it('runs one purchase at a time', async () => {
    const t = setup()
    await t.buy()
    await expect(t.buy()).rejects.toThrow(/already open/)
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

  it('answers shop.status for the open operation only', async () => {
    const t = setup()
    await t.buy()
    expect(await t.shop.request('shop.status', { operation: '0f0e0d0c-0000-4000-8000-000000000001' })).toMatchObject({ state: 'reviewing' })
    expect(await t.shop.request('shop.status', { operation: '0f0e0d0c-0000-4000-8000-000000000999' })).toBeNull()
  })
})
