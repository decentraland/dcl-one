// The shell purchase paid in Polygon MANA (behind `shop-mana-in-world`) on the web Shop's real rails: the executor's
// runManaGroup, its relayed and direct paths, the MANA guard, and the attempt kept at each checkpoint
// (shell/shop/manaPurchase). The Shop server, the chain, the relay and the wallet are the checkout's fakes.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { maxUint256, type Hex } from 'viem'
import { createFakeChain, createFakeRelay, createFakeShop, createFakeWallet, creatorTrade } from '../../shop/checkout/fakes'
import { ASSET_TYPE } from '../../shop/checkout/purchase/encoding'
import type { CatalogItem } from '@shop/discovery/lib/catalogItem'
import type { UsdCents } from '@shop/model/money'
import type { PurchaseOffer } from '@shop/model/purchase'
import type { Session } from '@shop/model/settlement'
import { BOOK, ITEM, serverOffer, storeMint, storePrice } from '@shop/payment/fixtures'
import { itemAsset } from '@shop/model/asset'
import { CHAIN } from '@shop/model/chain'
import type { StoredLogin } from '@explorer-core/features/auth/sso'
import { SHOP_PROTOCOL, type ShopOperationEvent } from '@dcl/one-ui/apps/shop/shopProtocol'
import { createShellShop, type ShellPurchaseHandlers, type ShellPurchaseView, type ShellShopServices } from '../shell/shop/purchase'

const BUYER = BOOK.buyer.toLowerCase()
const CONTRACT = BOOK.collection.toLowerCase()
const MANA = BOOK.mana.toLowerCase()
const STORE = BOOK.store.contract.toLowerCase()
const PRICE = '2300000000000000000'
const WEI = 10n ** 18n

const LOGIN: StoredLogin = {
  address: BUYER,
  identity: {
    ephemeralIdentity: { address: '0x' + '1'.repeat(40), publicKey: '0x04', privateKey: '0x' + 'ab'.repeat(32) },
    expiration: new Date(Date.now() + 3_600_000).toISOString(),
    authChain: [{ type: 'SIGNER', payload: BUYER, signature: '' }]
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
  category: 'wearable',
  wearableCategory: 'upper_body',
  available: 10
} as unknown as CatalogItem

/** An item only MANA can buy: its offer names no Credits venue. */
const manaOnly = (session: Session): PurchaseOffer =>
  serverOffer(
    { asset: itemAsset(CHAIN.POLYGON, BOOK.collection, ITEM), kind: 'wearable', sale: 'primaryMint', price: { manaWei: storePrice('wearable') }, venues: { mana: BOOK.store }, call: { kind: 'storeBuy' }, available: 10 },
    session
  )

/** The creator's listing of the item on the marketplace, priced in MANA itself (not USD-pegged). */
const manaListingOffer = (session: Session): PurchaseOffer =>
  serverOffer(
    { asset: itemAsset(CHAIN.POLYGON, BOOK.collection, ITEM), kind: 'wearable', sale: 'primaryMint', price: { manaWei: storePrice('wearable') }, venues: { mana: BOOK.marketplace }, call: { kind: 'accept' }, available: 10 },
    session
  )
const manaTrade = () => creatorTrade({ received: [{ assetType: ASSET_TYPE.ERC20, contractAddress: BOOK.mana, amount: PRICE, extra: '0x', beneficiary: BOOK.creator }] })

/** A Storage in memory; `refuse` makes chosen writes throw, as a full or blocked storage does. */
function memoryStorage(refuse: (key: string, value: string) => boolean = () => false): Storage {
  const map = new Map<string, string>()
  return {
    get length() {
      return map.size
    },
    clear: () => map.clear(),
    getItem: (k) => map.get(k) ?? null,
    key: (i) => [...map.keys()][i] ?? null,
    removeItem: (k) => void map.delete(k),
    setItem: (k, v) => {
      if (refuse(k, v)) throw new DOMException('full', 'QuotaExceededError')
      map.set(k, v)
    }
  }
}

afterEach(() => {
  vi.unstubAllEnvs()
})

type World = ReturnType<typeof world>

/** The fakes a purchase runs against, shared by the pages (shells) made over them, as a reload shares them. */
function world(opts: { gasless?: boolean; allowance?: bigint; walletChain?: number } = {}) {
  vi.stubEnv('VITE_SHOP_CHAIN_ID', '137')
  const chain = createFakeChain()
  if (opts.allowance != null) chain.allowances.set(`137:${BUYER}:${STORE}`, opts.allowance)
  const relay = createFakeRelay({ chain, confirm: 'success' })
  const shop = createFakeShop({})
  const wallet = createFakeWallet({
    ...(opts.walletChain != null ? { chainId: opts.walletChain } : {}),
    account: BUYER,
    onSend: (tx) => {
      const hash = `0x${'e'.repeat(62)}${(wallet.sent.length + 16).toString(16)}` as Hex
      chain.receipts.set(hash, 'success')
      if (tx.to.toLowerCase() === MANA) chain.allowances.set(`137:${BUYER}:${STORE}`, maxUint256)
      return hash
    }
  })
  return { chain, relay, shop, wallet, storage: memoryStorage(), gasless: opts.gasless ?? true }
}

function page(
  w: World,
  opts: {
    kind?: 'managed' | 'web3'
    flag?: boolean
    manaBalance?: bigint | 'fail'
    creditsCents?: number
    offer?: (session: Session) => PurchaseOffer
    priceWei?: () => string
    trade?: () => ReturnType<typeof creatorTrade> | null
    storage?: Storage
    services?: Partial<ShellShopServices>
    creditsBalance?: () => Promise<{ balanceCents: UsdCents; credits: number }>
  } = {}
) {
  const shown: { view: ShellPurchaseView; handlers: ShellPurchaseHandlers }[] = []
  const events: ShopOperationEvent[] = []
  const manaReads: number[] = []
  let clock = 0
  const cents = opts.creditsCents ?? 7000
  const shell = createShellShop({
    login: () => LOGIN,
    wallet: { kind: () => opts.kind ?? 'web3', provider: () => w.wallet },
    ui: { show: (view, handlers) => void shown.push({ view, handlers }), hide: () => {} },
    post: (e) => void events.push(e),
    openExternal: vi.fn(),
    signIn: vi.fn(),
    services: {
      client: () => w.shop.client,
      fetchItem: async () => LISTING,
      offers: () => async (queries, session) => Object.fromEntries(queries.map((q) => [q.key, (opts.offer ?? storeMint)(session)])),
      balance: opts.creditsBalance ?? (async () => ({ balanceCents: cents as UsdCents, credits: cents / 10 })),
      resolvers: () => ({ trade: async () => opts.trade?.() ?? null, store: async () => ({ priceWei: opts.priceWei?.() ?? PRICE, available: 10 }) }),
      reader: () => w.chain,
      primeFlags: async () => {},
      claim: async () => () => {},
      creatorName: async () => null,
      manager: (chainId) => ({ chainId, address: '0x' + 'de'.repeat(20), name: 'Decentraland Credits', version: '1.0.0' }),
      rail: () => ({
        relayerUrl: 'https://node.test/transactions-api/v1',
        fetchImpl: w.relay.fetchImpl,
        gasless: w.gasless,
        wait: { timeoutMs: 30, pollMs: 10, now: () => clock, sleep: async (ms) => void (clock += ms) }
      }),
      storage: () => opts.storage ?? w.storage,
      sleep: () => new Promise(() => {}),
      manaInWorld: () => opts.flag ?? true,
      manaBalance: async (_buyer, chainId) => {
        manaReads.push(chainId)
        if (opts.manaBalance === 'fail') throw new Error('rpc down')
        return opts.manaBalance ?? 40n * WEI
      },
      ...opts.services
    }
  })
  const until = async (step: ShellPurchaseView['step']) => {
    await vi.waitFor(() => expect(shown.at(-1)?.view.step).toBe(step), { timeout: 3000 })
    return shown.at(-1)!
  }
  const open = (cart = false) => shell.request('shop.buy', { v: SHOP_PROTOCOL, items: [{ contract: CONTRACT, itemId: String(ITEM), quantity: 1 }], method: 'credits', ...(cart ? { cart: true } : {}) })
  /** BUY with MANA picked on Choose your payment method. */
  const payMana = async () => {
    await open()
    ;(await until('method')).handlers.onPay?.('mana')
  }
  return { shell, shown, events, manaReads, until, open, payMana }
}

const relayedTo = (w: World) => w.relay.posts.map((p) => String((p as { transactionData: { params: unknown[] } }).transactionData.params[0]).toLowerCase())
const signatures = (w: World) => w.wallet.requests.filter((q) => q.method === 'eth_signTypedData_v4')

describe('the in-world purchase with MANA', () => {
  describe('when it is offered', () => {
    it('leaves the dialog as it was with the flag off: no MANA read, Buy Item', async () => {
      const w = world()
      const p = page(w, { flag: false })
      await p.open()
      await p.until('confirm')
      expect(p.manaReads).toEqual([])
      expect(p.shown.some((s) => s.view.step === 'method')).toBe(false)
    })

    it.each([
      ['an in-app wallet', { kind: 'managed' as const }, false],
      ['a cart', {}, true]
    ])('isn’t offered for %s', async (_what, opts, cart) => {
      const w = world()
      const p = page(w, opts)
      await p.open(cart)
      await p.until('confirm')
      expect(p.manaReads).toEqual([])
    })

    it('leaves MANA out when the buyer’s MANA can’t be read', async () => {
      const w = world()
      const p = page(w, { manaBalance: 'fail' })
      await p.open()
      await p.until('confirm')
      expect(p.manaReads).toEqual([137])
    })

    it('leaves MANA out for a buyer holding none while the Credits pay, as the web Shop does', async () => {
      const w = world()
      const p = page(w, { manaBalance: 0n })
      await p.open()
      await p.until('confirm')
    })

    it('offers both ways with their balances, the sale’s MANA price as quoted', async () => {
      const w = world()
      const p = page(w)
      await p.open()
      const { view } = await p.until('method')
      expect(view).toMatchObject({ step: 'method', credits: { price: 12, balance: 700, pays: true }, mana: { price: BigInt(PRICE), balance: 40n * WEI, pays: true } })
    })

    it('shows MANA the buyer holds that can’t pay, beside Credits that do', async () => {
      const w = world()
      const p = page(w, { manaBalance: WEI })
      await p.open()
      const { view } = await p.until('method')
      expect(view).toMatchObject({ credits: { pays: true }, mana: { pays: false, balance: WEI } })
    })

    it('lets a buyer whose Credits can’t buy it buy with MANA', async () => {
      const w = world({ allowance: maxUint256 })
      const p = page(w, { offer: manaOnly })
      await p.open()
      const { view, handlers } = await p.until('method')
      expect(view).toMatchObject({ credits: { price: null, pays: false }, mana: { pays: true } })
      handlers.onPay?.('mana')
      await p.until('success')
      expect(p.events.at(-1)).toMatchObject({ state: 'succeeded' })
    })

    it('lets MANA pay when the Credits balance can’t be read, the Credits row unpickable with its balance unknown', async () => {
      const w = world({ allowance: maxUint256 })
      const p = page(w, { creditsBalance: async () => Promise.reject(new Error('credits down')) })
      await p.open()
      const { view, handlers } = await p.until('method')
      expect(view).toMatchObject({ credits: { balance: null, pays: false }, mana: { pays: true } })
      handlers.onConfirm?.()
      handlers.onPay?.('credits')
      expect(w.shop.log.some((q) => q.name === 'authorizeCreditBatch')).toBe(false)
      handlers.onPay?.('mana')
      await p.until('success')
    })

    it('says the terms couldn’t be read when neither the Credits balance reads nor MANA pays', async () => {
      const w = world()
      const p = page(w, { manaBalance: WEI, creditsBalance: async () => Promise.reject(new Error('credits down')) })
      await p.open()
      const failed = await p.until('failed')
      expect(failed.view).toMatchObject({ step: 'failed', retry: true, message: "We couldn't check this item's price. Try again in a moment." })
    })

    it('doesn’t offer MANA for a listing paid in more than one entry, even when the first is the price', async () => {
      const w = world()
      const two = creatorTrade({
        received: [
          { assetType: ASSET_TYPE.ERC20, contractAddress: BOOK.mana, amount: PRICE, extra: '0x', beneficiary: BOOK.creator },
          { assetType: ASSET_TYPE.ERC20, contractAddress: BOOK.mana, amount: '1', extra: '0x', beneficiary: BOOK.creator }
        ]
      })
      const p = page(w, { offer: manaListingOffer, trade: () => two })
      await p.open()
      await p.until('failed')
      expect(p.shown.some((x) => x.view.step === 'method')).toBe(false)
    })

    it('still buys with Credits when they are picked', async () => {
      const w = world()
      const p = page(w)
      await p.open()
      ;(await p.until('method')).handlers.onPay?.('credits')
      // The Credits path runs as before (it asks the Shop for a credit), and nothing reaches MANA's rails.
      await vi.waitFor(() => expect(w.shop.log.some((q) => q.name === 'authorizeCreditBatch')).toBe(true))
      expect(relayedTo(w)).not.toContain(MANA)
    })
  })

  describe('authorizing MANA', () => {
    it('asks a first purchase at the venue for the allowance, relays the approve, then the sale', async () => {
      const w = world()
      const p = page(w)
      await p.payMana()
      const auth = await p.until('authorize')
      expect(auth.view).toMatchObject({ step: 'authorize', venue: 'the Collection Store', network: 'Polygon', required: BigInt(PRICE), state: 'ready' })
      auth.handlers.onAuthorize?.()
      await p.until('success')
      // The approve went through the relay, not as a send from the wallet: the purchase's relay transport lets it pass.
      expect(relayedTo(w)).toEqual([MANA, STORE])
      expect(w.wallet.sent).toHaveLength(0)
      expect(p.events.at(-1)).toMatchObject({ state: 'succeeded' })
    })

    it('doesn’t ask again once the allowance stands: one signature, one relayed sale', async () => {
      const w = world({ allowance: maxUint256 })
      const p = page(w)
      await p.payMana()
      await p.until('success')
      expect(p.shown.some((s) => s.view.step === 'authorize')).toBe(false)
      expect(signatures(w)).toHaveLength(1)
      expect(relayedTo(w)).toEqual([STORE])
    })

    it('never starts the sale after a cancel, however late the approval confirms', async () => {
      const w = world()
      let release!: () => void
      const held = new Promise<void>((r) => (release = r))
      const slow = (async (url: string, init?: RequestInit) => {
        await held
        return w.relay.fetchImpl(url, init)
      }) as unknown as typeof fetch
      const p = page(w, { services: { rail: () => ({ relayerUrl: 'https://node.test/transactions-api/v1', fetchImpl: slow, gasless: true }) } })
      await p.payMana()
      const auth = await p.until('authorize')
      auth.handlers.onAuthorize?.()
      await p.until('authorize')
      await vi.waitFor(() => expect(signatures(w)).toHaveLength(1))
      // Cancelled while the approve is on its way: it may still land, the sale never goes.
      p.shown.at(-1)!.handlers.onClose()
      await vi.waitFor(() => expect(p.events.at(-1)).toMatchObject({ state: 'cancelled' }))
      release()
      await new Promise((r) => setTimeout(r, 50))
      expect(relayedTo(w)).toEqual([MANA])
      expect(signatures(w)).toHaveLength(1)
      expect(w.wallet.sent).toHaveLength(0)
    })

    it('moves on to processing once the allowance is set, and a late CANCEL from that step can’t end a sale on its way', async () => {
      const w = world()
      let release!: () => void
      const held = new Promise<void>((r) => (release = r))
      // The sale's relay request waits; its receipt never comes (the relay took it and nothing more is known).
      w.relay.state.confirm = 'none'
      const slow = (async (url: string, init?: RequestInit) => {
        const target = String((JSON.parse(String(init?.body)) as { transactionData: { params: unknown[] } }).transactionData.params[0]).toLowerCase()
        if (target === STORE) await held
        return w.relay.fetchImpl(url, init)
      }) as unknown as typeof fetch
      const p = page(w, { services: { rail: () => ({ relayerUrl: 'https://node.test/transactions-api/v1', fetchImpl: slow, gasless: true, wait: { timeoutMs: 30, pollMs: 10 } }) } })
      await p.payMana()
      const auth = await p.until('authorize')
      auth.handlers.onAuthorize?.()
      // The approve relayed and the sale signed: its relay request is the one held.
      await vi.waitFor(() => expect(signatures(w)).toHaveLength(2))
      expect(relayedTo(w)).toEqual([MANA])
      expect(p.shown.at(-1)?.view.step).toBe('processing')
      // The step's own CANCEL, pressed late (a stale view): nothing.
      auth.handlers.onClose()
      release()
      await p.until('pending')
      expect(relayedTo(w)).toEqual([MANA, STORE])
      expect(p.events.at(-1)).toMatchObject({ state: 'pending' })
      const op = p.events.at(-1)!.operation
      p.shown.at(-1)!.handlers.onClose()
      expect(await p.open()).toMatchObject({ operation: op, state: 'pending' })
      expect(signatures(w)).toHaveLength(2)
    })

    it.each([
      ['the first allowance read fails', (calls: number) => (calls === 1 ? 'fail' : 0n)],
      ['the allowance reads enough, then none', (calls: number) => (calls === 1 ? maxUint256 : 0n)]
    ] as const)('never approves without the buyer’s AUTHORIZE when %s', async (_what, allowance) => {
      const w = world()
      const call = w.chain.call.bind(w.chain)
      let reads = 0
      w.chain.call = async (chainId, to, data) => {
        if (data.startsWith('0xdd62ed3e')) {
          const answer = allowance(++reads)
          if (answer === 'fail') throw new Error('rpc down')
          w.chain.allowances.set(`137:${BUYER}:${STORE}`, answer)
        }
        return call(chainId, to, data)
      }
      const p = page(w)
      await p.payMana()
      const failed = await p.until('failed')
      expect(failed.view).toMatchObject({ step: 'failed', retry: true })
      expect(p.shown.some((x) => x.view.step === 'authorize')).toBe(false)
      // No approve reached the wallet: no signature of MANA's domain, no send to MANA; and nothing went to the relay.
      const approveSigned = signatures(w).some((q) => String(q.params?.[1]).toLowerCase().includes(MANA))
      expect(approveSigned).toBe(false)
      expect(w.wallet.sent).toHaveLength(0)
      expect(w.relay.posts).toHaveLength(0)
    })

    it('says a declined approval, and AUTHORIZE tries again', async () => {
      const w = world()
      const p = page(w)
      await p.payMana()
      w.wallet.next = 'reject'
      ;(await p.until('authorize')).handlers.onAuthorize?.()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'authorize', state: 'failed', message: 'You declined the authorization in your wallet.' }))
      p.shown.at(-1)!.handlers.onAuthorize?.()
      await p.until('success')
    })
  })

  describe('sending the sale', () => {
    it('sends it once from the wallet after the relay proves it sent nothing', async () => {
      const w = world({ allowance: maxUint256 })
      w.relay.state.mode = 'refuse'
      const p = page(w)
      await p.payMana()
      await p.until('success')
      expect(relayedTo(w)).toEqual([STORE])
      expect(w.wallet.sent.map((t) => t.to.toLowerCase())).toEqual([STORE])
    })

    it('offers a funded wallet on another network the switch after the relay’s quota refusal, saying why', async () => {
      const w = world({ allowance: maxUint256, walletChain: 1 })
      Object.assign(w.relay.state, { mode: 'refuse', refuseStatus: 429, refuseBody: { error: 'quota_reached', quota: 30 } })
      const p = page(w)
      await p.payMana()
      const failed = await p.until('failed')
      expect(failed.view).toMatchObject({ step: 'failed', retry: true, switchTo: 'Polygon' })
      expect((failed.view as { message: string }).message).toBe(
        "You've used all 30 fee-free transactions this account gets in 24 hours. Try again later. You can switch to Polygon and pay the network fee with your wallet. Nothing was bought."
      )
      expect(w.wallet.sent).toHaveLength(0)
    })

    it('tells an unfunded wallet on another network the relay’s reason, with no talk of networks', async () => {
      const w = world({ allowance: maxUint256, walletChain: 1 })
      Object.assign(w.relay.state, { mode: 'refuse', refuseStatus: 429, refuseBody: { error: 'quota_reached', quota: 30 } })
      w.chain.balance = async () => 10n ** 16n
      const p = page(w)
      await p.payMana()
      const failed = await p.until('failed')
      expect(failed.view).toMatchObject({ step: 'failed', retry: true, message: "You've used all 30 fee-free transactions this account gets in 24 hours. Try again later. Nothing was bought." })
      expect(failed.view).not.toHaveProperty('switchTo')
    })

    it.each([
      ['a refusal without the relay’s evidence', (w: World) => Object.assign(w.relay.state, { mode: 'refuse', refuseEvidence: false })],
      ['an answer lost after the relay took it', (w: World) => Object.assign(w.relay.state, { mode: 'fail-after-send', failStatus: 504 })],
      ['a relay that doesn’t answer', (w: World) => Object.assign(w.relay.state, { mode: 'down' })]
    ])('follows a sale after %s, and never sends it from the wallet', async (_what, set) => {
      const w = world({ allowance: maxUint256 })
      set(w)
      const p = page(w)
      await p.payMana()
      const pending = await p.until('pending')
      expect(pending.view).toMatchObject({ step: 'pending', check: 'ready' })
      expect(w.wallet.sent).toHaveLength(0)
      expect(relayedTo(w)).toEqual([STORE])
    })

    it('follows a direct send whose answer was lost through a reload, and a second BUY sends nothing more', async () => {
      const w = world({ gasless: false, allowance: maxUint256 })
      const first = page(w)
      w.wallet.next = 'fail'
      await first.payMana()
      const pending = await first.until('pending')
      expect(pending.view).toMatchObject({ step: 'pending', check: 'ready', message: expect.stringContaining('may have gone out') })
      const op = first.events.at(-1)!.operation
      expect(first.events.at(-1)).toMatchObject({ state: 'pending' })
      const sends = w.wallet.requests.filter((q) => q.method === 'eth_sendTransaction').length
      // CHECK AGAIN has nothing to read for it: it stays.
      pending.handlers.onCheckAgain?.()
      await vi.waitFor(() => expect(first.shown.at(-1)?.view).toMatchObject({ step: 'pending', check: 'ready' }))
      // A reload, even with the flag off: the same purchase, still pending, and nothing sent again.
      const again = page(w, { flag: false })
      const answer = await again.open()
      expect(answer).toMatchObject({ operation: op, state: 'pending' })
      expect(again.shown.at(-1)?.view).toMatchObject({ step: 'pending', check: 'ready' })
      expect(w.wallet.requests.filter((q) => q.method === 'eth_sendTransaction')).toHaveLength(sends)
    })

    it('ends a direct send the buyer declined in the wallet as not bought, with TRY AGAIN', async () => {
      const w = world({ gasless: false, allowance: maxUint256 })
      const p = page(w)
      w.wallet.next = 'reject'
      await p.payMana()
      const failed = await p.until('failed')
      expect(failed.view).toMatchObject({ step: 'failed', retry: true })
      expect(p.events.at(-1)).toMatchObject({ state: 'reviewing' })
    })

    it.each([
      ['the signature’s checkpoint', '"stage":"signing"'],
      ['the relay request’s checkpoint', '"stage":"dispatched"']
    ])('sends nothing on either way when %s can’t be kept', async (_what, marker) => {
      const w = world({ allowance: maxUint256 })
      const storage = memoryStorage((key, value) => key.includes(':record:') && value.includes(marker))
      const p = page(w, { storage })
      await p.payMana()
      const failed = await p.until('failed')
      expect(failed.view).toMatchObject({ step: 'failed', retry: true })
      expect(relayedTo(w)).toEqual([])
      expect(w.wallet.sent).toHaveLength(0)
    })

    it('buys a creator’s listing priced in MANA, through the marketplace', async () => {
      const w = world()
      w.chain.allowances.set(`137:${BUYER}:${BOOK.marketplace.contract.toLowerCase()}`, maxUint256)
      // One signed listing, read the same at the quote and at BUY (the fixture stamps its times when built).
      const listing = manaTrade()
      const p = page(w, { offer: manaListingOffer, trade: () => listing })
      await p.payMana()
      await p.until('success')
      expect(relayedTo(w)).toEqual([BOOK.marketplace.contract.toLowerCase()])
    })

    it('asks for a new confirmation when the listing it would accept isn’t the one shown, even at the same price', async () => {
      const w = world()
      w.chain.allowances.set(`137:${BUYER}:${BOOK.marketplace.contract.toLowerCase()}`, maxUint256)
      let reads = 0
      // The same price, another signed listing: only the sale's own bytes tell them apart.
      const shown = manaTrade()
      const other = { ...shown, signature: `0x${'cd'.repeat(65)}` }
      const p = page(w, { offer: manaListingOffer, trade: () => (++reads === 1 ? shown : other) })
      await p.payMana()
      const failed = await p.until('failed')
      expect(failed.view).toMatchObject({ step: 'failed', retry: true, message: 'Its price or terms changed. Try again to see the new ones. No MANA was spent.' })
      expect(w.wallet.requests.some((q) => q.method === 'eth_signTypedData_v4' || q.method === 'eth_sendTransaction')).toBe(false)
      expect(w.relay.posts).toHaveLength(0)
    })

    it('asks for a new confirmation when the sale changed since it was shown', async () => {
      const w = world({ allowance: maxUint256 })
      let reads = 0
      const p = page(w, { priceWei: () => (++reads === 1 ? PRICE : '2500000000000000000') })
      await p.payMana()
      const failed = await p.until('failed')
      expect(failed.view).toMatchObject({ step: 'failed', retry: true })
      expect((failed.view as { message: string }).message).toMatch(/price/i)
      expect(w.wallet.requests.some((q) => q.method === 'eth_signTypedData_v4' || q.method === 'eth_sendTransaction')).toBe(false)
    })
  })

  describe('a reload', () => {
    it('ends a purchase whose sale never left the page (only its signature was kept) as cancelled', async () => {
      const w = world({ allowance: maxUint256 })
      // The relay request's checkpoint never sticks: the page goes with the signature kept and nothing sent.
      const storage = memoryStorage((key, value) => key.includes(':record:') && value.includes('"stage":"dispatched"'))
      const first = page(w, { storage })
      await first.payMana()
      await first.until('failed')
      const op = first.events.at(-1)!.operation
      const again = page(w, { storage })
      const list = (await again.shell.request('shop.status', {})) as { operation: string; state: string }[]
      expect(list.find((o) => o.operation === op)?.state).not.toBe('pending')
    })

    /** A MANA purchase this browser kept mid-way, as a page that went would leave it. */
    const seed = (w: World, id: string, rails: { rail: 'relay' | 'direct'; nonce?: string; stage: string }[], txHash?: string) =>
      w.storage.setItem(
        `dcl-inworld-shop:record:v1:${BUYER}:${id}`,
        JSON.stringify({
          id,
          buyer: BUYER,
          items: [{ contract: CONTRACT, itemId: String(ITEM), quantity: 1 }],
          state: 'processing',
          chainId: 137,
          ...(txHash ? { txHash } : {}),
          mana: { venue: STORE, fingerprint: '0x' + 'ab'.repeat(32), rails },
          createdAt: 1,
          updatedAt: 1
        })
      )

    it('ends a purchase none of whose ways was dispatched as cancelled: it provably sent nothing', async () => {
      const w = world()
      seed(w, '0000000a-0000-0000-0000-000000000001', [{ rail: 'relay', nonce: '0', stage: 'signing' }])
      const p = page(w)
      await p.shell.request('shop.status', {})
      await vi.waitFor(() => expect(p.events.at(-1)).toMatchObject({ operation: '0000000a-0000-0000-0000-000000000001', state: 'cancelled' }))
    })

    it('keeps a dispatched purchase with no hash as it is, and a BUY of its item shows it', async () => {
      const w = world()
      seed(w, '0000000a-0000-0000-0000-000000000002', [{ rail: 'relay', nonce: '0', stage: 'dispatched' }])
      const p = page(w)
      const answer = await p.open()
      expect(answer).toMatchObject({ operation: '0000000a-0000-0000-0000-000000000002', state: 'processing' })
      expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', check: 'ready', message: expect.stringContaining('may have gone out') })
      await new Promise((r) => setTimeout(r, 30))
      expect(p.events.some((e) => e.operation === '0000000a-0000-0000-0000-000000000002' && e.state !== 'processing')).toBe(false)
      expect(w.wallet.requests.some((q) => q.method === 'eth_signTypedData_v4' || q.method === 'eth_sendTransaction')).toBe(false)
    })

    it('ends a purchase by the receipt of its own hash', async () => {
      const w = world()
      const hash = '0x' + 'f'.repeat(63) + '3'
      w.chain.receipts.set(hash, 'success')
      seed(w, '0000000a-0000-0000-0000-000000000003', [{ rail: 'relay', nonce: '0', stage: 'dispatched' }], hash)
      const p = page(w)
      await p.shell.request('shop.status', {})
      await vi.waitFor(() => expect(p.events.at(-1)).toMatchObject({ operation: '0000000a-0000-0000-0000-000000000003', state: 'succeeded', txHash: hash }))
    })

    it('keeps reading a Credits purchase’s record, which has no MANA attempt, as before', async () => {
      const w = world()
      const p = page(w, { flag: false })
      await p.open()
      await p.until('confirm')
      const op = p.events.at(-1)!.operation
      const stored = JSON.parse(w.storage.getItem([...Array(w.storage.length).keys()].map((i) => w.storage.key(i)!).find((k) => k.includes(':record:') && k.endsWith(op))!)!) as Record<string, unknown>
      expect(stored.mana).toBeUndefined()
      const again = page(w, { flag: false })
      const list = (await again.shell.request('shop.status', {})) as { operation: string }[]
      expect(list.map((o) => o.operation)).toContain(op)
    })
  })
})
