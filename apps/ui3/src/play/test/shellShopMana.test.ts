// The shell purchase paid in Polygon MANA (behind `shop-mana-in-world`) on the web Shop's real rails: the executor's
// runManaGroup, its relayed and direct paths, the MANA guard, and the attempt kept at each checkpoint
// (shell/shop/manaPurchase). The Shop server, the chain, the relay and the wallet are the checkout's fakes.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { decodeFunctionData, maxUint256, parseAbi, type Hex } from 'viem'
import { createFakeChain, createFakeRelay, createFakeShop, createFakeWallet, creatorTrade } from '../../shop/checkout/fakes'
import { ASSET_TYPE, encodeStoreBuy } from '../../shop/checkout/purchase/encoding'
import { manaFingerprint } from '../shell/shop/manaPurchase'
import type { CatalogItem } from '@shop/discovery/lib/catalogItem'
import type { UsdCents } from '@shop/model/money'
import type { PurchaseOffer } from '@shop/model/purchase'
import type { Session } from '@shop/model/settlement'
import { BOOK, ITEM, serverOffer, storeMint, storePrice } from '@shop/payment/fixtures'
import { itemAsset } from '@shop/model/asset'
import { CHAIN } from '@shop/model/chain'
import type { StoredLogin } from '@explorer-core/features/auth/sso'
import { SHOP_PROTOCOL, type ShopOperationEvent } from '@dcl/one-ui/apps/shop/shopProtocol'
import { createShellShop, RETRY_AFTER_MS, type ShellPurchaseHandlers, type ShellPurchaseView, type ShellShopServices } from '../shell/shop/purchase'
import type { ManaEvidence, ManaExecution } from '../shell/shop/manaEvidence'

const BUYER = BOOK.buyer.toLowerCase()
const CONTRACT = BOOK.collection.toLowerCase()
const MANA = BOOK.mana.toLowerCase()
const STORE = BOOK.store.contract.toLowerCase()
const PRICE = '2300000000000000000'
const WEI = 10n ** 18n
const EXECUTE_ABI = parseAbi(['function executeMetaTransaction(address userAddress, bytes functionSignature, bytes32 sigR, bytes32 sigS, uint8 sigV) payable returns (bytes)'])

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
  // What the relay put on chain: each hash's target and signed input, as the chain would answer for it.
  const txs = new Map<string, { to: string; input: Hex }>()
  const relay = createFakeRelay({
    chain,
    confirm: 'success',
    onRelay: (hash, body) => {
      const [to, input] = (body as { transactionData: { params: [string, Hex] } }).transactionData.params
      // `foreignHash`: the relay answers the hash of some other transaction than the one it was given.
      txs.set(hash.toLowerCase(), { to, input: onChain.foreignHash ? ('0xdeadbeef' as Hex) : input })
    }
  })
  /** The chain as the evidence reads it: the nonce the fake chain holds, the executions given, the relay's transactions. */
  const onChain = {
    executions: [] as ManaExecution[],
    fail: false,
    finalized: 1000n,
    latest: 1000n,
    receiptBlock: 900n,
    foreignHash: false,
    /** Holds every finalized-block read after the first (the pre-dispatch checkpoint) until released. */
    hold: null as Promise<void> | null,
    finalizedReads: 0
  }
  const evidence: ManaEvidence = {
    finalized: async () => {
      onChain.finalizedReads++
      if (onChain.hold && onChain.finalizedReads > 1) await onChain.hold
      return onChain.finalized
    },
    latest: async () => onChain.latest,
    nonceAt: async (_c, venue, user) => chain.nonces.get(`${venue.address.toLowerCase()}:${user.toLowerCase()}`) ?? 0n,
    // As eth_getLogs answers: only what lies in the range asked for.
    executions: async (_c, _venue, from, to) => {
      if (onChain.fail) throw new Error('range too large')
      return onChain.executions.filter((e) => e.blockNumber >= from && e.blockNumber <= to)
    },
    transaction: async (_c, hash) => txs.get(hash.toLowerCase()) ?? null,
    receipt: async (_c, hash) => {
      const status = chain.receipts.get(hash.toLowerCase())
      return status ? { transactionHash: hash as Hex, status, blockNumber: onChain.receiptBlock } : null
    }
  }
  const clock = { now: 1_000_000 }
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
  return { chain, relay, shop, wallet, storage: memoryStorage(), gasless: opts.gasless ?? true, txs, onChain, evidence, clock }
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
    /** Another live tab holds every operation's lock. */
    foreign?: boolean
    /** The page's own flag reading (a host's `__SHOP_FLAGS__`, else the flags file), not a stub. */
    liveFlag?: boolean
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
      claim: async () => (opts.foreign ? null : () => {}),
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
      now: () => w.clock.now,
      manaEvidence: () => w.evidence,
      ...(opts.liveFlag ? {} : { manaInWorld: () => opts.flag ?? true }),
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

    it('reads the flag as a page does: off by default, on when the host declares it', async () => {
      const w = world()
      const off = page(w, { liveFlag: true })
      await off.open()
      await off.until('confirm')
      expect(off.manaReads).toEqual([])
      ;(window as Window & { __SHOP_FLAGS__?: Record<string, boolean> }).__SHOP_FLAGS__ = { manaInWorld: true }
      try {
        const on = page(world(), { liveFlag: true })
        await on.open()
        await on.until('method')
      } finally {
        delete (window as Window & { __SHOP_FLAGS__?: unknown }).__SHOP_FLAGS__
      }
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

    it('moves on once the allowance is set, and a late CANCEL from that step can’t end a sale on its way', async () => {
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
      // The approve relayed and the sale signed and kept as gone: its relay request is the one held, and the purchase
      // already shows as in progress, closable.
      await vi.waitFor(() => expect(signatures(w)).toHaveLength(2))
      expect(relayedTo(w)).toEqual([MANA])
      await vi.waitFor(() => expect(p.shown.at(-1)?.view.step).toBe('pending'))
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
    it('isn’t offered with the relay off: the sale goes only through the relay', async () => {
      const w = world({ gasless: false })
      const p = page(w)
      await p.open()
      await p.until('confirm')
    })

    it('sends nothing from the wallet when the relay refuses it, and says why, with TRY AGAIN', async () => {
      const w = world({ allowance: maxUint256 })
      Object.assign(w.relay.state, { mode: 'refuse', refuseStatus: 429, refuseBody: { error: 'quota_reached', quota: 30 } })
      const p = page(w)
      await p.payMana()
      const failed = await p.until('failed')
      expect(failed.view).toMatchObject({ step: 'failed', retry: true })
      expect((failed.view as { message: string }).message).toContain("You've used all 30 fee-free transactions this account gets in 24 hours.")
      expect(failed.view).not.toHaveProperty('switchTo')
      expect(w.wallet.sent).toHaveLength(0)
      expect(relayedTo(w)).toEqual([STORE])
    })

    it.each([
      ['a refusal without the relay’s evidence', (w: World) => Object.assign(w.relay.state, { mode: 'refuse', refuseEvidence: false })],
      ['an answer lost after the relay took it', (w: World) => Object.assign(w.relay.state, { mode: 'fail-after-send', failStatus: 504 })],
      ['a relay that doesn’t answer', (w: World) => Object.assign(w.relay.state, { mode: 'down' })]
    ])('follows a sale after %s, checking it on chain, and never sends it from the wallet', async (_what, set) => {
      const w = world({ allowance: maxUint256 })
      set(w)
      const p = page(w)
      await p.payMana()
      const pending = await p.until('pending')
      expect(pending.view).toMatchObject({ step: 'pending', check: 'ready', message: 'Checking your purchase. You can close this window.' })
      expect(w.wallet.sent).toHaveLength(0)
      expect(relayedTo(w)).toEqual([STORE])
    })

    it('ends a sale whose signature the buyer declined as not bought, with TRY AGAIN', async () => {
      const w = world({ allowance: maxUint256 })
      const p = page(w)
      w.wallet.next = 'reject'
      await p.payMana()
      const failed = await p.until('failed')
      expect(failed.view).toMatchObject({ step: 'failed', retry: true })
      expect(relayedTo(w)).toEqual([])
    })

    it.each([
      ['the signature’s checkpoint', '"stage":"signing"'],
      ['the relay request’s checkpoint', '"stage":"dispatched"']
    ])('sends nothing when %s can’t be kept', async (_what, marker) => {
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

  describe('a sale whose answer was lost', () => {
    const NONCE_KEY = `${STORE}:${BUYER}`
    /** The typed data's nonce of each sale signature the wallet was asked for. */
    const signedNonces = (w: World) =>
      signatures(w)
        .map((q) => JSON.parse(String(q.params?.[1])) as { domain: { verifyingContract: string }; message: { nonce: string } })
        .filter((t) => t.domain.verifyingContract.toLowerCase() === STORE)
        .map((t) => t.message.nonce)
    /** The first send, lost; then CHECK AGAIN past RETRY_AFTER_MS: the page now offers RETRY. */
    const lost = async (w: World, p: ReturnType<typeof page>) => {
      w.relay.state.mode = 'down'
      await p.payMana()
      const pending = await p.until('pending')
      w.relay.state.mode = 'ok'
      w.clock.now += RETRY_AFTER_MS
      pending.handlers.onCheckAgain?.()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', retry: true }))
      return p.shown.at(-1)!
    }

    it('ends bought once the chain shows the very execution it signed, and not bought once its nonce went elsewhere', async () => {
      const w = world({ allowance: maxUint256 })
      const p = page(w)
      w.relay.state.mode = 'fail-after-send'
      await p.payMana()
      const pending = await p.until('pending')
      // The relay ran it at N after all: the nonce moved, and the venue logged our call from the transaction we signed.
      const [hash, tx] = [...w.txs.entries()][0]
      w.chain.nonces.set(NONCE_KEY, 1n)
      w.onChain.executions = [{ user: BUYER, call: decodeCall(tx.input), txHash: hash as Hex, blockNumber: 1050n }]
      w.onChain.finalized = 1100n
      pending.handlers.onCheckAgain?.()
      await p.until('success')
      expect(p.events.at(-1)).toMatchObject({ state: 'succeeded', txHash: hash })

      const w2 = world({ allowance: maxUint256 })
      const p2 = page(w2)
      w2.relay.state.mode = 'down'
      await p2.payMana()
      const pending2 = await p2.until('pending')
      // N went to something else, and the venue logged nothing of ours: it can never run.
      w2.chain.nonces.set(NONCE_KEY, 1n)
      pending2.handlers.onCheckAgain?.()
      const failed = await p2.until('failed')
      expect(failed.view).toMatchObject({ message: "This purchase didn't go through. Nothing was bought and no MANA was spent." })
    })

    it('keeps checking while the chain can’t say: a range it can’t read, the same call it can’t attribute', async () => {
      const w = world({ allowance: maxUint256 })
      const p = page(w)
      w.relay.state.mode = 'down'
      await p.payMana()
      const pending = await p.until('pending')
      w.chain.nonces.set(NONCE_KEY, 2n)
      w.onChain.fail = true
      pending.handlers.onCheckAgain?.()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', check: 'ready' }))
      // The same call by the buyer, at N+1, signed elsewhere: not ours, and no RETRY (N is gone).
      w.onChain.fail = false
      w.onChain.executions = [{ user: BUYER, call: decodeCall((w.relay.posts[0] as { transactionData: { params: [string, Hex] } }).transactionData.params[1]), txHash: ('0x' + 'ab'.repeat(32)) as Hex, blockNumber: 1060n }]
      w.onChain.finalized = 1100n
      w.clock.now += RETRY_AFTER_MS
      p.shown.at(-1)!.handlers.onCheckAgain?.()
      await new Promise((r) => setTimeout(r, 30))
      expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', check: 'ready' })
      expect(p.events.at(-1)).toMatchObject({ state: 'pending' })
    })

    it('offers RETRY only after RETRY_AFTER_MS, and retries the same sale at the same nonce, through the relay only', async () => {
      const w = world({ allowance: maxUint256 })
      const p = page(w)
      w.relay.state.mode = 'down'
      await p.payMana()
      const pending = await p.until('pending')
      pending.handlers.onCheckAgain?.()
      await new Promise((r) => setTimeout(r, 30))
      expect(p.shown.at(-1)?.view).not.toHaveProperty('retry')
      w.relay.state.mode = 'ok'
      w.clock.now += RETRY_AFTER_MS
      p.shown.at(-1)!.handlers.onCheckAgain?.()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', retry: true, message: 'You can retry this same purchase fee-free. It can complete only once.' }))
      const op = p.events.at(-1)!.operation
      p.shown.at(-1)!.handlers.onRetry?.()
      await p.until('success')
      // Two signatures of the same sale, both at the first nonce; the second relayed; nothing from the wallet.
      expect(signedNonces(w)).toEqual(['0', '0'])
      expect(relayedTo(w)).toEqual([STORE, STORE])
      expect(w.wallet.sent).toHaveLength(0)
      expect(p.events.at(-1)).toMatchObject({ operation: op, state: 'succeeded' })
    })

    it('signs nothing when the first send landed just before the retry, and ends bought by it', async () => {
      const w = world({ allowance: maxUint256 })
      const p = page(w)
      const retry = await lost(w, p)
      // The first send ran meanwhile: the nonce moved, and its execution is the one this page signed.
      const firstPayload = (w.relay.posts[0] as { transactionData: { params: [string, Hex] } }).transactionData.params[1]
      const landed = ('0x' + '5a'.repeat(32)) as Hex
      w.txs.set(landed, { to: STORE, input: firstPayload })
      w.chain.receipts.set(landed, 'success')
      w.chain.nonces.set(NONCE_KEY, 1n)
      w.onChain.executions = [{ user: BUYER, call: decodeCall(firstPayload), txHash: landed, blockNumber: 1050n }]
      w.onChain.finalized = 1100n
      retry.handlers.onRetry?.()
      await p.until('success')
      expect(signedNonces(w)).toEqual(['0'])
      expect(relayedTo(w)).toEqual([STORE])
      expect(p.events.at(-1)).toMatchObject({ state: 'succeeded', txHash: landed })
    })

    it('settles a refused retry alone, and still ends bought by the first send', async () => {
      const w = world({ allowance: maxUint256 })
      // The watch reads the chain again on its own, every few milliseconds here.
      const p = page(w, { services: { sleep: () => new Promise((r) => setTimeout(r, 20)) } })
      const retry = await lost(w, p)
      w.relay.state.mode = 'refuse'
      retry.handlers.onRetry?.()
      await vi.waitFor(() => expect(relayedTo(w)).toEqual([STORE, STORE]))
      await vi.waitFor(() => expect(p.shown.at(-1)?.view.step).toBe('pending'))
      expect(p.events.at(-1)).toMatchObject({ state: 'pending' })
      // Later the first send turns up on chain.
      const firstPayload = (w.relay.posts[0] as { transactionData: { params: [string, Hex] } }).transactionData.params[1]
      const landed = ('0x' + '6b'.repeat(32)) as Hex
      w.txs.set(landed, { to: STORE, input: firstPayload })
      w.chain.receipts.set(landed, 'success')
      w.chain.nonces.set(NONCE_KEY, 1n)
      w.onChain.executions = [{ user: BUYER, call: decodeCall(firstPayload), txHash: landed, blockNumber: 1050n }]
      w.onChain.finalized = 1100n
      await p.until('success')
      expect(p.events.at(-1)).toMatchObject({ state: 'succeeded', txHash: landed })
    })

    it('offers RETRY after a reload, from what the record kept, and never to a tab that doesn’t hold the purchase', async () => {
      const w = world({ allowance: maxUint256 })
      const first = page(w)
      w.relay.state.mode = 'down'
      await first.payMana()
      await first.until('pending')
      const op = first.events.at(-1)!.operation
      w.relay.state.mode = 'ok'
      w.clock.now += RETRY_AFTER_MS
      // Another tab, the purchase's lock held elsewhere: shown, never retried.
      const other = page(w, { foreign: true, flag: false })
      expect(await other.open()).toMatchObject({ operation: op })
      await new Promise((r) => setTimeout(r, 30))
      expect(other.shown.some((x) => (x.view as { retry?: boolean }).retry)).toBe(false)
      // A reload that holds it, with the flag off: the same purchase, RETRY open, the same nonce.
      const again = page(w, { flag: false })
      expect(await again.open()).toMatchObject({ operation: op })
      await vi.waitFor(() => expect(again.shown.at(-1)?.view).toMatchObject({ step: 'pending', retry: true }))
      again.shown.at(-1)!.handlers.onRetry?.()
      await again.until('success')
      expect(signedNonces(w)).toEqual(['0', '0'])
    })
  })

  describe('settling only on the chain’s evidence', () => {
    const NONCE_KEY = `${STORE}:${BUYER}`

    it('doesn’t take the relay’s hash for a purchase when that transaction isn’t the one it signed', async () => {
      const w = world({ allowance: maxUint256 })
      w.onChain.foreignHash = true
      const p = page(w)
      await p.payMana()
      const pending = await p.until('pending')
      expect(pending.view).toMatchObject({ step: 'pending', message: 'Checking your purchase. You can close this window.' })
      expect(p.events.at(-1)).toMatchObject({ state: 'pending' })
      expect(p.shown.some((x) => x.view.step === 'success')).toBe(false)
    })

    it('doesn’t take a receipt past the finalized block, and settles once it is final', async () => {
      const w = world({ allowance: maxUint256 })
      w.onChain.receiptBlock = 1500n
      const p = page(w)
      await p.payMana()
      const pending = await p.until('pending')
      w.onChain.finalized = 1600n
      pending.handlers.onCheckAgain?.()
      await p.until('success')
    })

    it('doesn’t take a reverted transaction for the end of the sale: its nonce may still run it', async () => {
      const w = world({ allowance: maxUint256 })
      w.relay.state.confirm = 'reverted'
      const p = page(w)
      await p.payMana()
      await p.until('pending')
      expect(p.events.at(-1)).toMatchObject({ state: 'pending' })
    })

    it('finds a sale that landed below the head it was sent at (a reorg), from the finalized checkpoint', async () => {
      const w = world({ allowance: maxUint256 })
      // Sent while the head was at 1010 and the finalized block at 1000; it lands at 1005 after a reorg.
      w.onChain.latest = 1010n
      w.relay.state.mode = 'down'
      const p = page(w)
      await p.payMana()
      const pending = await p.until('pending')
      const payload = (w.relay.posts[0] as { transactionData: { params: [string, Hex] } }).transactionData.params[1]
      const landed = ('0x' + '7c'.repeat(32)) as Hex
      w.txs.set(landed, { to: STORE, input: payload })
      w.chain.receipts.set(landed, 'success')
      w.onChain.receiptBlock = 1005n
      w.onChain.finalized = 1020n
      w.chain.nonces.set(NONCE_KEY, 1n)
      w.onChain.executions = [{ user: BUYER, call: decodeCall(payload), txHash: landed, blockNumber: 1005n }]
      pending.handlers.onCheckAgain?.()
      await p.until('success')
    })

    it('keeps checking when the finalized block reads below the checkpoint, even with the nonce gone', async () => {
      const w = world({ allowance: maxUint256 })
      w.relay.state.mode = 'down'
      const p = page(w)
      await p.payMana()
      const pending = await p.until('pending')
      w.onChain.finalized = 990n
      w.chain.nonces.set(NONCE_KEY, 1n)
      pending.handlers.onCheckAgain?.()
      await new Promise((r) => setTimeout(r, 30))
      expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', check: 'ready' })
      expect(p.events.at(-1)).toMatchObject({ state: 'pending' })
    })

    it('shows the purchase in progress, closable, while the chain read after the send is held, and settles once it answers', async () => {
      const w = world({ allowance: maxUint256 })
      let release!: () => void
      w.onChain.hold = new Promise<void>((r) => (release = r))
      const p = page(w, { services: { sleep: () => new Promise((r) => setTimeout(r, 20)) } })
      await p.payMana()
      const pending = await p.until('pending')
      expect(p.events.at(-1)).toMatchObject({ state: 'pending' })
      // The read is still held: closing works, and BUY shows the same purchase without signing or sending again.
      await vi.waitFor(() => expect(w.onChain.finalizedReads).toBeGreaterThan(1))
      pending.handlers.onClose()
      const op = p.events.at(-1)!.operation
      expect(await p.open()).toMatchObject({ operation: op, state: 'pending' })
      expect(p.shown.at(-1)?.view.step).toBe('pending')
      expect(signatures(w)).toHaveLength(1)
      expect(w.relay.posts).toHaveLength(1)
      release()
      await p.until('success')
      expect(p.events.at(-1)).toMatchObject({ operation: op, state: 'succeeded' })
    })

    it('shows the purchase in progress once the sale is kept as gone, while the executor still waits on its receipt', async () => {
      const w = world({ allowance: maxUint256 })
      let release!: () => void
      const held = new Promise<void>((r) => (release = r))
      const reader = { ...w.chain, receipt: async (...a: Parameters<typeof w.chain.receipt>) => (await held, w.chain.receipt(...a)) }
      const p = page(w, { services: { reader: () => reader } })
      await p.payMana()
      const pending = await p.until('pending')
      expect(w.relay.posts).toHaveLength(1)
      // Closed while the receipt is held, then the same item again: the same purchase, nothing signed or sent again.
      pending.handlers.onClose()
      const op = p.events.at(-1)!.operation
      expect(await p.open()).toMatchObject({ operation: op, state: 'pending' })
      expect(signatures(w)).toHaveLength(1)
      expect(w.relay.posts).toHaveLength(1)
      release()
      await vi.waitFor(() => expect(p.events.at(-1)).toMatchObject({ operation: op, state: 'succeeded' }))
    })

    it('shows a retry in progress once it is kept as gone, while the relay’s answer to it is held', async () => {
      const w = world({ allowance: maxUint256 })
      let release!: () => void
      const held = new Promise<void>((r) => (release = r))
      let posts = 0
      const slow = (async (url: string, init?: RequestInit) => {
        // Counted as asked: the first request (the relay down) throws, and the retry's is the second.
        const n = ++posts
        const res = await w.relay.fetchImpl(url, init)
        if (n === 2) await held
        return res
      }) as unknown as typeof fetch
      const p = page(w, { services: { rail: () => ({ relayerUrl: 'https://node.test/transactions-api/v1', fetchImpl: slow, gasless: true, wait: { timeoutMs: 30, pollMs: 10 } }) } })
      w.relay.state.mode = 'down'
      await p.payMana()
      const pending = await p.until('pending')
      w.relay.state.mode = 'ok'
      w.clock.now += RETRY_AFTER_MS
      pending.handlers.onCheckAgain?.()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', retry: true }))
      p.shown.at(-1)!.handlers.onRetry?.()
      await vi.waitFor(() => expect(w.relay.posts).toHaveLength(2))
      await vi.waitFor(() => expect(p.shown.at(-1)?.view.step).toBe('pending'))
      p.shown.at(-1)!.handlers.onClose()
      const op = p.events.at(-1)!.operation
      expect(await p.open()).toMatchObject({ operation: op, state: 'pending' })
      expect(signatures(w)).toHaveLength(2)
      expect(w.relay.posts).toHaveLength(2)
      release()
      await vi.waitFor(() => expect(p.events.at(-1)).toMatchObject({ operation: op, state: 'succeeded' }))
    })

    it('keeps the relay’s late refusal with the purchase it answers: closed meanwhile, it ends unbought and frees the item', async () => {
      const w = world({ allowance: maxUint256 })
      let release!: () => void
      const held = new Promise<void>((r) => (release = r))
      const slow = (async (url: string, init?: RequestInit) => {
        const res = await w.relay.fetchImpl(url, init)
        await held
        return res
      }) as unknown as typeof fetch
      w.relay.state.mode = 'refuse'
      const p = page(w, { services: { rail: () => ({ relayerUrl: 'https://node.test/transactions-api/v1', fetchImpl: slow, gasless: true, wait: { timeoutMs: 30, pollMs: 10 } }) } })
      await p.payMana()
      const pending = await p.until('pending')
      const op = p.events.at(-1)!.operation
      pending.handlers.onClose()
      release()
      await vi.waitFor(() => expect(p.events.at(-1)).toMatchObject({ operation: op, state: 'failed' }))
      // Its item can be bought again: a fresh purchase, not the one that ended.
      const again = (await p.open()) as { operation: string }
      expect(again.operation).not.toBe(op)
      await p.until('method')
    })

    it('shows the purchase in progress at once after a retry, before the chain answers', async () => {
      const w = world({ allowance: maxUint256 })
      const p = page(w, { services: { sleep: () => new Promise((r) => setTimeout(r, 20)) } })
      w.relay.state.mode = 'down'
      await p.payMana()
      const pending = await p.until('pending')
      w.relay.state.mode = 'ok'
      w.clock.now += RETRY_AFTER_MS
      pending.handlers.onCheckAgain?.()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', retry: true }))
      let release!: () => void
      w.onChain.hold = new Promise<void>((r) => (release = r))
      w.onChain.finalizedReads = 1
      p.shown.at(-1)!.handlers.onRetry?.()
      await vi.waitFor(() => expect(w.relay.posts).toHaveLength(2))
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending' }))
      release()
      await p.until('success')
    })

    it('opens RETRY on a dialog already up once the wait passes, with no press', async () => {
      const w = world({ allowance: maxUint256 })
      w.relay.state.mode = 'down'
      const p = page(w, { services: { sleep: () => new Promise((r) => setTimeout(r, 20)) } })
      await p.payMana()
      await p.until('pending')
      await new Promise((r) => setTimeout(r, 60))
      expect(p.shown.at(-1)?.view).not.toHaveProperty('retry')
      w.clock.now += RETRY_AFTER_MS
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', retry: true }))
    })
  })

  describe('a retry that didn’t go', () => {
    const FIRST = { stage: 'dispatched', payload: '0x' + 'c1'.repeat(32), at: 0 }
    const seedRecord = (w: World, id: string, sends: Record<string, unknown>[]) =>
      w.storage.setItem(
        `dcl-inworld-shop:record:v1:${BUYER}:${id}`,
        JSON.stringify({
          id,
          buyer: BUYER,
          items: [{ contract: CONTRACT, itemId: String(ITEM), quantity: 1 }],
          state: 'pending',
          chainId: 137,
          mana: {
            venue: STORE,
            terms: { kind: 'mints', venue: STORE, units: [{ contract: CONTRACT, itemId: String(ITEM) }], maxWei: PRICE },
            fingerprint: manaFingerprint({ kind: 'mints', venue: STORE, units: [{ contract: CONTRACT, itemId: String(ITEM) }], maxWei: BigInt(PRICE) }),
            nonce: '0',
            data: encodeStoreBuy([{ collection: CONTRACT, itemId: String(ITEM), priceWei: PRICE }], BUYER),
            fromBlock: '900',
            sends
          },
          createdAt: 1,
          updatedAt: 1
        })
      )
    const stored = (w: World, id: string) => JSON.parse(w.storage.getItem(`dcl-inworld-shop:record:v1:${BUYER}:${id}`)!) as { mana: { sends: { stage: string }[] } }

    it('drops a retry left signing by a page that went, keeps the first send’s evidence, and offers RETRY again', async () => {
      const w = world({ allowance: maxUint256 })
      w.clock.now = RETRY_AFTER_MS + 10
      seedRecord(w, '0000000b-0000-0000-0000-000000000001', [FIRST, { stage: 'signing' }])
      const p = page(w)
      expect(await p.open()).toMatchObject({ operation: '0000000b-0000-0000-0000-000000000001' })
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', retry: true }))
      expect(stored(w, '0000000b-0000-0000-0000-000000000001').mana.sends).toEqual([FIRST])
      p.shown.at(-1)!.handlers.onRetry?.()
      await vi.waitFor(() => expect(p.events.at(-1)).toMatchObject({ operation: '0000000b-0000-0000-0000-000000000001', state: 'succeeded' }))
    })

    it('offers RETRY again after a retry whose signature failed in the wallet before it was sent', async () => {
      const w = world({ allowance: maxUint256 })
      w.clock.now = RETRY_AFTER_MS + 10
      seedRecord(w, '0000000b-0000-0000-0000-000000000002', [FIRST])
      const p = page(w)
      await p.open()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', retry: true }))
      w.wallet.next = 'fail'
      p.shown.at(-1)!.handlers.onRetry?.()
      await vi.waitFor(() => expect(stored(w, '0000000b-0000-0000-0000-000000000002').mana.sends).toEqual([FIRST]))
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', retry: true }))
      expect(w.relay.posts).toHaveLength(0)
      p.shown.at(-1)!.handlers.onRetry?.()
      await vi.waitFor(() => expect(p.events.at(-1)).toMatchObject({ operation: '0000000b-0000-0000-0000-000000000002', state: 'succeeded' }))
    })

    it('stops offering RETRY when the record holds as many sends as it keeps, and still checks the chain', async () => {
      const w = world({ allowance: maxUint256 })
      w.clock.now = RETRY_AFTER_MS + 10
      const sends = Array.from({ length: 8 }, (_, i) => ({ stage: 'dispatched', payload: '0x' + (i + 16).toString(16).repeat(32), at: 0 }))
      seedRecord(w, '0000000b-0000-0000-0000-000000000003', sends)
      const p = page(w)
      await p.open()
      await vi.waitFor(() =>
        expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', check: 'ready', message: "This purchase can't be retried again here. We're still checking it on chain, and it will show up here once it settles." })
      )
      expect(p.shown.some((x) => (x.view as { retry?: boolean }).retry)).toBe(false)
    })
  })

  describe('a reload', () => {
    const FINGERPRINT = '0x' + 'ab'.repeat(32)
    /** A MANA purchase this browser kept mid-way, as a page that went would leave it. */
    const seed = (w: World, id: string, sends: Record<string, unknown>[]) =>
      w.storage.setItem(
        `dcl-inworld-shop:record:v1:${BUYER}:${id}`,
        JSON.stringify({
          id,
          buyer: BUYER,
          items: [{ contract: CONTRACT, itemId: String(ITEM), quantity: 1 }],
          state: 'processing',
          chainId: 137,
          mana: {
            venue: STORE,
            terms: { kind: 'mints', venue: STORE, units: [{ contract: CONTRACT, itemId: String(ITEM) }], maxWei: PRICE },
            fingerprint: FINGERPRINT,
            nonce: '0',
            data: '0xa4fdc78a',
            fromBlock: '900',
            sends
          },
          createdAt: 1,
          updatedAt: 1
        })
      )

    it('ends a purchase none of whose sends was dispatched as cancelled: it provably sent nothing', async () => {
      const w = world()
      seed(w, '0000000a-0000-0000-0000-000000000001', [{ stage: 'signing' }])
      const p = page(w)
      await p.shell.request('shop.status', {})
      await vi.waitFor(() => expect(p.events.at(-1)).toMatchObject({ operation: '0000000a-0000-0000-0000-000000000001', state: 'cancelled' }))
    })

    it('keeps a dispatched purchase the chain can’t settle yet, and a BUY of its item shows it', async () => {
      const w = world()
      seed(w, '0000000a-0000-0000-0000-000000000002', [{ stage: 'dispatched', payload: '0x' + 'cd'.repeat(32), at: w.clock.now }])
      const p = page(w)
      const answer = await p.open()
      expect(answer).toMatchObject({ operation: '0000000a-0000-0000-0000-000000000002', state: 'processing' })
      expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', message: 'Checking your purchase. You can close this window.' })
      await new Promise((r) => setTimeout(r, 30))
      expect(p.events.some((e) => e.operation === '0000000a-0000-0000-0000-000000000002' && e.state !== 'processing')).toBe(false)
      expect(w.wallet.requests.some((q) => q.method === 'eth_signTypedData_v4' || q.method === 'eth_sendTransaction')).toBe(false)
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

/** The venue's call inside a signed executeMetaTransaction (the CollectionStore's: from, call, r, s, v). */
function decodeCall(payload: Hex): Hex {
  return decodeFunctionData({ abi: EXECUTE_ABI, data: payload }).args[1] as Hex
}
