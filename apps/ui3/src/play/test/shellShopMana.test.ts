// The shell purchase paid in Polygon MANA (behind `shop-mana-in-world`) on the web Shop's real rails: the executor's
// runManaGroup, its relayed and direct paths, the MANA guard, and the attempt kept at each checkpoint
// (shell/shop/manaPurchase). The Shop server, the chain, the relay and the wallet are the checkout's fakes.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { decodeFunctionData, keccak256, maxUint256, parseAbi, type Hex } from 'viem'
import { createFakeChain, createFakeRelay, createFakeShop, createFakeWallet, creatorTrade } from '../../shop/checkout/fakes'
import { ASSET_TYPE, encodeStoreBuy } from '../../shop/checkout/purchase/encoding'
import { encodeExecuteMetaTransaction } from '../../shop/checkout/chain/metatx'
import { manaFingerprint } from '../shell/shop/manaPurchase'
import { createLedger } from '../shell/shop/ledger'
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
function world(opts: { gasless?: boolean; allowance?: bigint; walletChain?: number; advancing?: boolean } = {}) {
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
      // `advancing`: it lands just past the latest finalized block read (its sale's checkpoint).
      if (opts.advancing) onChain.blocks.set(hash.toLowerCase(), onChain.head + 1n)
    }
  })
  /**
   * The chain as the evidence reads it: the nonce the fake chain holds now, the executions a log read returns, the
   * relay's transactions. As at a verified venue, each execution moved its signer's nonce once, so the nonce at a block
   * is today's less the executions after it (`unlogged` ones too: on chain, but left out of every log read).
   */
  const onChain = {
    executions: [] as ManaExecution[],
    unlogged: [] as ManaExecution[],
    fail: false,
    /** The finalized block the first read answers: the pre-dispatch checkpoint. */
    checkpoint: 1000n,
    /** The finalized block every later read answers. */
    finalized: 1100n,
    /** Where the relay's transactions land: past the checkpoint, and final. */
    receiptBlock: 1050n,
    foreignHash: false,
    /** Holds every finalized-block read after the first (the pre-dispatch checkpoint) until released. */
    hold: null as Promise<void> | null,
    finalizedReads: 0,
    /** Holds the next finalized-block read only, until released. */
    holdNext: null as Promise<void> | null,
    /**
     * `advancing` (several sales, each with its own checkpoint): every finalized-block read is past the one before
     * (`head`), unless `frozen` (finality lagging), and each relayed transaction lands at its own block (`blocks`).
     */
    head: 1000n,
    frozen: false,
    blocks: new Map<string, bigint>()
  }
  const evidence: ManaEvidence = {
    finalized: async () => {
      onChain.finalizedReads++
      const next = onChain.holdNext
      onChain.holdNext = null
      if (next) await next
      if (onChain.hold && onChain.finalizedReads > 1) await onChain.hold
      if (opts.advancing) {
        if (!onChain.frozen) onChain.head += 100n
        return onChain.head
      }
      return onChain.finalizedReads === 1 ? onChain.checkpoint : onChain.finalized
    },
    nonceAt: async (_c, venue, user, block) => {
      const now = chain.nonces.get(`${venue.address.toLowerCase()}:${user.toLowerCase()}`) ?? 0n
      const after = [...onChain.executions, ...onChain.unlogged].filter((e) => e.user === user.toLowerCase() && e.blockNumber > block)
      return now - BigInt(after.length)
    },
    // As eth_getLogs answers: only what lies in the range asked for.
    executions: async (_c, _venue, from, to) => {
      if (onChain.fail) throw new Error('range too large')
      return onChain.executions.filter((e) => e.blockNumber >= from && e.blockNumber <= to)
    },
    transaction: async (_c, hash) => txs.get(hash.toLowerCase()) ?? null,
    receipt: async (_c, hash) => {
      const status = chain.receipts.get(hash.toLowerCase())
      return status ? { transactionHash: hash as Hex, status, blockNumber: onChain.blocks.get(hash.toLowerCase()) ?? onChain.receiptBlock } : null
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
    /** Who is signed in now (LOGIN by default). */
    login?: () => StoredLogin | null
  } = {}
) {
  const shown: { view: ShellPurchaseView; handlers: ShellPurchaseHandlers }[] = []
  const events: ShopOperationEvent[] = []
  const manaReads: number[] = []
  let clock = 0
  const cents = opts.creditsCents ?? 7000
  const shell = createShellShop({
    login: opts.login ?? (() => LOGIN),
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
/** The typed data's nonce of each sale signature the wallet was asked for. */
const signedNonces = (w: World) =>
  signatures(w)
    .map((q) => JSON.parse(String(q.params?.[1])) as { domain: { verifyingContract: string }; message: { nonce: string } })
    .filter((t) => t.domain.verifyingContract.toLowerCase() === STORE)
    .map((t) => t.message.nonce)

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

    it('isn’t offered for an in-app wallet', async () => {
      const w = world()
      const p = page(w, { kind: 'managed' })
      await p.open()
      await p.until('confirm')
      expect(p.manaReads).toEqual([])
    })

    it('is offered for a cart, as for one item', async () => {
      const w = world()
      const p = page(w)
      await p.open(true)
      const { view } = await p.until('method')
      expect(view).toMatchObject({ mana: { price: BigInt(PRICE), pays: true } })
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
      w.onChain.executions = [{ user: BUYER, call: decodeCall(tx.input), txHash: hash as Hex, blockNumber: 1050n, logIndex: 0 }]
      w.onChain.finalized = 1100n
      pending.handlers.onCheckAgain?.()
      await p.until('success')
      expect(p.events.at(-1)).toMatchObject({ state: 'succeeded', txHash: hash })

      const w2 = world({ allowance: maxUint256 })
      const p2 = page(w2)
      w2.relay.state.mode = 'down'
      await p2.payMana()
      const pending2 = await p2.until('pending')
      // N went to another call by the buyer, and nothing of ours ran: it can never run.
      w2.chain.nonces.set(NONCE_KEY, 1n)
      w2.onChain.executions = [{ user: BUYER, call: '0xbeef', txHash: ('0x' + 'cd'.repeat(32)) as Hex, blockNumber: 1050n, logIndex: 3 }]
      w2.onChain.finalized = 1100n
      pending2.handlers.onCheckAgain?.()
      const failed = await p2.until('failed')
      expect(failed.view).toMatchObject({ message: "This purchase didn't go through. Nothing was bought and no MANA was spent." })
    })

    it('keeps checking while the chain can’t say: a range it can’t read, a read that leaves an execution out', async () => {
      const w = world({ allowance: maxUint256 })
      const p = page(w)
      w.relay.state.mode = 'down'
      await p.payMana()
      const pending = await p.until('pending')
      // N went to another call at 1060, then N+1 to this call, signed elsewhere.
      w.chain.nonces.set(NONCE_KEY, 2n)
      w.onChain.unlogged = [{ user: BUYER, call: '0xbeef', txHash: ('0x' + 'cd'.repeat(32)) as Hex, blockNumber: 1060n, logIndex: 0 }]
      w.onChain.executions = [{ user: BUYER, call: decodeCall((w.relay.posts[0] as { transactionData: { params: [string, Hex] } }).transactionData.params[1]), txHash: ('0x' + 'ab'.repeat(32)) as Hex, blockNumber: 1060n, logIndex: 1 }]
      w.onChain.finalized = 1100n
      // The block's logs can't be read.
      w.onChain.fail = true
      pending.handlers.onCheckAgain?.()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', check: 'ready' }))
      // Read, they leave out the execution at N and return only N+1: one execution where the nonce moved twice, so it
      // decides nothing, and offers no RETRY (N is gone).
      w.onChain.fail = false
      w.clock.now += RETRY_AFTER_MS
      p.shown.at(-1)!.handlers.onCheckAgain?.()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', check: 'ready' }))
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
      // One signature, at the first nonce; the retry relayed exactly what it signed, with nothing asked of the wallet.
      expect(signedNonces(w)).toEqual(['0'])
      expect(relayedTo(w)).toEqual([STORE, STORE])
      expect(w.relay.posts[1]).toEqual(w.relay.posts[0])
      expect(w.wallet.sent).toHaveLength(0)
      expect(p.events.at(-1)).toMatchObject({ operation: op, state: 'succeeded' })
    })

    it('waits RETRY_AFTER_MS again after a retry that got no answer, from that retry', async () => {
      const w = world({ allowance: maxUint256 })
      const p = page(w)
      const retry = await lost(w, p)
      w.relay.state.mode = 'down'
      retry.handlers.onRetry?.()
      await vi.waitFor(() => expect(w.relay.posts).toHaveLength(2))
      await vi.waitFor(() => expect(p.shown.at(-1)?.view.step).toBe('pending'))
      // Just short of the wait after the retry (the first send went long before): CHECK AGAIN offers no RETRY yet.
      w.clock.now += RETRY_AFTER_MS - 1
      p.shown.at(-1)!.handlers.onCheckAgain?.()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', check: 'ready' }))
      expect(p.shown.at(-1)?.view).not.toHaveProperty('retry')
      w.clock.now += 1
      p.shown.at(-1)!.handlers.onCheckAgain?.()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', retry: true }))
    })

    it('waits RETRY_AFTER_MS again after a retry the relay refused, from that refused request', async () => {
      const w = world({ allowance: maxUint256 })
      const p = page(w)
      const retry = await lost(w, p)
      w.relay.state.mode = 'refuse'
      retry.handlers.onRetry?.()
      await vi.waitFor(() => expect(w.relay.posts).toHaveLength(2))
      await vi.waitFor(() => expect(p.shown.at(-1)?.view.step).toBe('pending'))
      // The refused request left the record, and still counts: CHECK AGAIN at once offers no RETRY.
      p.shown.at(-1)!.handlers.onCheckAgain?.()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', check: 'ready' }))
      expect(p.shown.at(-1)?.view).not.toHaveProperty('retry')
      w.relay.state.mode = 'ok'
      w.clock.now += RETRY_AFTER_MS
      p.shown.at(-1)!.handlers.onCheckAgain?.()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', retry: true }))
      expect(w.relay.posts).toHaveLength(2)
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
      w.onChain.executions = [{ user: BUYER, call: decodeCall(firstPayload), txHash: landed, blockNumber: 1050n, logIndex: 0 }]
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
      w.onChain.executions = [{ user: BUYER, call: decodeCall(firstPayload), txHash: landed, blockNumber: 1050n, logIndex: 0 }]
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
      // From what the record kept: the signed execution itself, sent again with no new signature.
      expect(signedNonces(w)).toEqual(['0'])
      expect(w.relay.posts[1]).toEqual(w.relay.posts[0])
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
      w.onChain.executions = [{ user: BUYER, call: decodeCall(payload), txHash: landed, blockNumber: 1005n, logIndex: 0 }]
      pending.handlers.onCheckAgain?.()
      await p.until('success')
    })

    it('ends not bought when another call ran at its nonce while the wallet’s prompt was open, before the checkpoint', async () => {
      const w = world({ allowance: maxUint256 })
      w.relay.state.mode = 'down'
      // The first finalized read is the checkpoint, at the dispatch: by then another call by the buyer ran at N (block 990),
      // after the nonce was read for the signature.
      const finalized = w.evidence.finalized
      let first = true
      w.evidence.finalized = async (c) => {
        if (first) {
          first = false
          w.chain.nonces.set(NONCE_KEY, 1n)
          w.onChain.executions = [{ user: BUYER, call: '0xbeef', txHash: ('0x' + '9b'.repeat(32)) as Hex, blockNumber: 990n, logIndex: 0 }]
        }
        return finalized(c)
      }
      const p = page(w)
      await p.payMana()
      // Found at once, reaching back past the checkpoint: no send of this sale can ever run.
      const failed = await p.until('failed')
      expect(signedNonces(w)).toEqual(['0'])
      expect(failed.view).toMatchObject({ message: "This purchase didn't go through. Nothing was bought and no MANA was spent." })
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
      expect(signatures(w)).toHaveLength(1)
      expect(w.relay.posts).toHaveLength(2)
      release()
      await vi.waitFor(() => expect(p.events.at(-1)).toMatchObject({ operation: op, state: 'succeeded' }))
    })

    it('offers RETRY after a send that never got an answer, at the same nonce, without waiting on that answer', async () => {
      const w = world({ allowance: maxUint256 })
      let n = 0
      // The first sale request reaches the relay and its answer never comes back.
      const stalled = (async (url: string, init?: RequestInit) => {
        const first = ++n === 1
        const res = await w.relay.fetchImpl(url, init)
        if (first) await new Promise(() => {})
        return res
      }) as unknown as typeof fetch
      const p = page(w, {
        services: {
          rail: () => ({ relayerUrl: 'https://node.test/transactions-api/v1', fetchImpl: stalled, gasless: true, wait: { timeoutMs: 30, pollMs: 10 } }),
          manaTimeouts: () => ({ sendMs: 50, readMs: 50 })
        }
      })
      w.relay.state.confirm = 'none'
      await p.payMana()
      const pending = await p.until('pending')
      w.relay.state.confirm = 'success'
      w.clock.now += RETRY_AFTER_MS
      // The send gave way at its deadline: RETRY opens on CHECK AGAIN, the stalled answer still outstanding.
      await new Promise((r) => setTimeout(r, 120))
      pending.handlers.onCheckAgain?.()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', retry: true }))
      p.shown.at(-1)!.handlers.onRetry?.()
      await vi.waitFor(() => expect(p.events.at(-1)).toMatchObject({ state: 'succeeded' }))
      expect(signedNonces(w)).toEqual(['0'])
      expect(w.relay.posts).toHaveLength(2)
      expect(w.relay.posts[1]).toEqual(w.relay.posts[0])
    })

    it('offers RETRY after a send whose answer’s body never ends, without closing it or reloading', async () => {
      const w = world({ allowance: maxUint256 })
      let n = 0
      // The first sale request is taken and answered with headers at once; its body never ends.
      const endless = (async (url: string, init?: RequestInit) => {
        const res = await w.relay.fetchImpl(url, init)
        if (++n === 1) return new Response(new ReadableStream<Uint8Array>({ start: () => {} }), { status: 200 })
        return res
      }) as unknown as typeof fetch
      const p = page(w, {
        services: {
          rail: () => ({ relayerUrl: 'https://node.test/transactions-api/v1', fetchImpl: endless, gasless: true, wait: { timeoutMs: 30, pollMs: 10 } }),
          manaTimeouts: () => ({ sendMs: 50, readMs: 50 })
        }
      })
      w.relay.state.confirm = 'none'
      await p.payMana()
      const pending = await p.until('pending')
      w.relay.state.confirm = 'success'
      w.clock.now += RETRY_AFTER_MS
      await new Promise((r) => setTimeout(r, 120))
      pending.handlers.onCheckAgain?.()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', retry: true }))
      p.shown.at(-1)!.handlers.onRetry?.()
      await vi.waitFor(() => expect(p.events.at(-1)).toMatchObject({ state: 'succeeded' }))
    })

    it('keeps the relay’s late refusal on the dialog still up for it, with TRY AGAIN, however the watch wakes', async () => {
      const w = world({ allowance: maxUint256 })
      let release!: () => void
      const held = new Promise<void>((r) => (release = r))
      const slow = (async (url: string, init?: RequestInit) => {
        const res = await w.relay.fetchImpl(url, init)
        await held
        return res
      }) as unknown as typeof fetch
      // The watch sleeps until woken.
      const sleepers: (() => void)[] = []
      const wake = () => sleepers.splice(0).forEach((r) => r())
      w.relay.state.mode = 'refuse'
      const p = page(w, {
        services: {
          rail: () => ({ relayerUrl: 'https://node.test/transactions-api/v1', fetchImpl: slow, gasless: true, wait: { timeoutMs: 30, pollMs: 10 } }),
          sleep: () => new Promise<void>((r) => sleepers.push(r))
        }
      })
      await p.payMana()
      await p.until('pending')
      const op = p.events.at(-1)!.operation
      release()
      const failed = await p.until('failed')
      expect(failed.view).toMatchObject({ step: 'failed', retry: true })
      wake()
      await new Promise((r) => setTimeout(r, 30))
      wake()
      await new Promise((r) => setTimeout(r, 30))
      expect(p.shown.at(-1)?.view).toMatchObject({ step: 'failed', retry: true })
      expect(p.events.filter((e) => e.operation === op).map((e) => e.state)).not.toContain('cancelled')
      // TRY AGAIN: a fresh attempt; and the refused one ends failed, once.
      p.shown.at(-1)!.handlers.onRetry?.()
      await p.until('method')
      expect(p.events.filter((e) => e.operation === op && (e.state === 'failed' || e.state === 'cancelled')).map((e) => e.state)).toEqual(['failed'])
    })

    it.each([
      ['on the dialog still up for it, with TRY AGAIN', true],
      ['quietly when the dialog was closed', false]
    ])('ends a sale the relay refused past the send’s deadline as failed %s, never as cancelled', async (_what, open) => {
      const w = world({ allowance: maxUint256 })
      let release!: () => void
      const held = new Promise<void>((r) => (release = r))
      const slow = (async (url: string, init?: RequestInit) => {
        const res = await w.relay.fetchImpl(url, init)
        await held
        return res
      }) as unknown as typeof fetch
      const sleepers: (() => void)[] = []
      const wake = () => sleepers.splice(0).forEach((r) => r())
      w.relay.state.mode = 'refuse'
      const p = page(w, {
        services: {
          rail: () => ({ relayerUrl: 'https://node.test/transactions-api/v1', fetchImpl: slow, gasless: true, wait: { timeoutMs: 30, pollMs: 10 } }),
          sleep: () => new Promise<void>((r) => sleepers.push(r)),
          manaTimeouts: () => ({ sendMs: 50, readMs: 50 })
        }
      })
      await p.payMana()
      const pending = await p.until('pending')
      const op = p.events.at(-1)!.operation
      if (!open) pending.handlers.onClose()
      // Past the deadline: the rails took it as a relay that didn't answer; then the refusal comes back.
      await new Promise((r) => setTimeout(r, 120))
      release()
      await new Promise((r) => setTimeout(r, 30))
      wake()
      await vi.waitFor(() => expect(p.events.filter((e) => e.operation === op).at(-1)?.state).toMatch(open ? /reviewing/ : /failed/))
      expect(p.events.filter((e) => e.operation === op).map((e) => e.state)).not.toContain('cancelled')
      if (open) {
        expect(p.shown.at(-1)?.view).toMatchObject({ step: 'failed', retry: true, message: 'This purchase couldn’t go out. Nothing was bought and no MANA was spent.' })
        wake()
        await new Promise((r) => setTimeout(r, 30))
        expect(p.shown.at(-1)?.view).toMatchObject({ step: 'failed', retry: true })
        p.shown.at(-1)!.handlers.onRetry?.()
        await p.until('method')
        expect(p.events.filter((e) => e.operation === op && (e.state === 'failed' || e.state === 'cancelled')).map((e) => e.state)).toEqual(['failed'])
      } else {
        expect((await p.open()) as { operation: string }).not.toMatchObject({ operation: op })
      }
    })

    it('leaves the refusal the watch showed, with TRY AGAIN, when an older CHECK AGAIN read comes back afterwards', async () => {
      const w = world({ allowance: maxUint256 })
      let answer!: () => void
      const held = new Promise<void>((r) => (answer = r))
      const slow = (async (url: string, init?: RequestInit) => {
        const res = await w.relay.fetchImpl(url, init)
        await held
        return res
      }) as unknown as typeof fetch
      const sleepers: (() => void)[] = []
      const wake = () => sleepers.splice(0).forEach((r) => r())
      w.relay.state.mode = 'refuse'
      const p = page(w, {
        services: {
          rail: () => ({ relayerUrl: 'https://node.test/transactions-api/v1', fetchImpl: slow, gasless: true, wait: { timeoutMs: 30, pollMs: 10 } }),
          sleep: () => new Promise<void>((r) => sleepers.push(r)),
          manaTimeouts: () => ({ sendMs: 50, readMs: 50 })
        }
      })
      await p.payMana()
      const pending = await p.until('pending')
      await new Promise((r) => setTimeout(r, 120))
      // CHECK AGAIN's chain read is held; meanwhile the refusal comes back and the watch shows it.
      let read!: () => void
      w.onChain.holdNext = new Promise<void>((r) => (read = r))
      pending.handlers.onCheckAgain?.()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', check: 'checking' }))
      answer()
      await new Promise((r) => setTimeout(r, 30))
      wake()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'failed', retry: true }))
      read()
      await new Promise((r) => setTimeout(r, 30))
      expect(p.shown.at(-1)?.view).toMatchObject({ step: 'failed', retry: true })
    })

    it('ends failed, never cancelled, when the first send and its retry are both refused', async () => {
      const w = world({ allowance: maxUint256 })
      const holds: (() => void)[] = []
      const slow = (async (url: string, init?: RequestInit) => {
        const res = await w.relay.fetchImpl(url, init)
        await new Promise<void>((r) => holds.push(r))
        return res
      }) as unknown as typeof fetch
      w.relay.state.mode = 'refuse'
      const p = page(w, {
        services: {
          rail: () => ({ relayerUrl: 'https://node.test/transactions-api/v1', fetchImpl: slow, gasless: true, wait: { timeoutMs: 30, pollMs: 10 } }),
          manaTimeouts: () => ({ sendMs: 80, readMs: 80 })
        }
      })
      await p.payMana()
      const pending = await p.until('pending')
      const op = p.events.at(-1)!.operation
      // A's answer is held past its deadline; then RETRY sends B.
      await new Promise((r) => setTimeout(r, 160))
      w.clock.now += RETRY_AFTER_MS
      pending.handlers.onCheckAgain?.()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', retry: true }))
      p.shown.at(-1)!.handlers.onRetry?.()
      await vi.waitFor(() => expect(holds).toHaveLength(2))
      // A's refusal comes back while B is in flight, then B's own, in time.
      holds[0]()
      await new Promise((r) => setTimeout(r, 10))
      holds[1]()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'failed', retry: true }))
      expect(p.events.filter((e) => e.operation === op).map((e) => e.state)).not.toContain('cancelled')
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
    const CALL = encodeStoreBuy([{ collection: CONTRACT, itemId: String(ITEM), priceWei: PRICE }], BUYER)
    /** The signed execution the first send carried, as the record keeps it. */
    const SIGNED = encodeExecuteMetaTransaction('functionSignature', BUYER, CALL, `0x${'22'.repeat(64)}1c`)
    const FIRST = { id: 's1', stage: 'dispatched', payload: keccak256(SIGNED), at: 0 }
    const seedRecord = (w: World, id: string, sends: Record<string, unknown>[]) =>
      w.storage.setItem(
        `dcl-inworld-shop:record:mana-v1:${BUYER}:${id}`,
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
            data: CALL,
            fromBlock: '900',
            signed: SIGNED,
            sends
          },
          createdAt: 1,
          updatedAt: 1
        })
      )
    const stored = (w: World, id: string) => JSON.parse(w.storage.getItem(`dcl-inworld-shop:record:mana-v1:${BUYER}:${id}`)!) as { mana: { sends: { stage: string }[] } }

    it('drops a retry left signing by a page that went, keeps the first send’s evidence, and offers RETRY again', async () => {
      const w = world({ allowance: maxUint256 })
      w.clock.now = RETRY_AFTER_MS + 10
      seedRecord(w, '0000000b-0000-0000-0000-000000000001', [FIRST, { id: 's2', stage: 'signing' }])
      const p = page(w)
      expect(await p.open()).toMatchObject({ operation: '0000000b-0000-0000-0000-000000000001' })
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', retry: true }))
      expect(stored(w, '0000000b-0000-0000-0000-000000000001').mana.sends).toEqual([FIRST])
      p.shown.at(-1)!.handlers.onRetry?.()
      await vi.waitFor(() => expect(p.events.at(-1)).toMatchObject({ operation: '0000000b-0000-0000-0000-000000000001', state: 'succeeded' }))
      // What the record kept went again, as it was signed, and the wallet was asked nothing.
      expect((w.relay.posts[0] as { transactionData: { params: [string, Hex] } }).transactionData.params).toEqual([STORE, SIGNED])
      expect(w.wallet.requests).toHaveLength(0)
    })

    it('offers RETRY again after a retry that couldn’t read the nonce, with nothing sent', async () => {
      const w = world({ allowance: maxUint256 })
      w.clock.now = RETRY_AFTER_MS + 10
      seedRecord(w, '0000000b-0000-0000-0000-000000000002', [FIRST])
      // The chain's reads fail while `failing` holds: the retry can't tell its nonce is still unused.
      let failing = false
      const reader = { ...w.chain, call: async (...a: Parameters<typeof w.chain.call>) => (failing ? Promise.reject(new Error('rate limited')) : w.chain.call(...a)) }
      const p = page(w, { services: { reader: () => reader } })
      await p.open()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', retry: true }))
      failing = true
      p.shown.at(-1)!.handlers.onRetry?.()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view.step).toBe('pending'))
      failing = false
      await vi.waitFor(() => expect(stored(w, '0000000b-0000-0000-0000-000000000002').mana.sends).toEqual([FIRST]))
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', retry: true }))
      expect(w.relay.posts).toHaveLength(0)
      p.shown.at(-1)!.handlers.onRetry?.()
      await vi.waitFor(() => expect(p.events.at(-1)).toMatchObject({ operation: '0000000b-0000-0000-0000-000000000002', state: 'succeeded' }))
    })

    it('sends nothing for a buyer no longer signed in here, at the click or while the nonce is read', async () => {
      const w = world({ allowance: maxUint256 })
      w.clock.now = RETRY_AFTER_MS + 10
      seedRecord(w, '0000000b-0000-0000-0000-000000000004', [FIRST])
      let who: StoredLogin | null = LOGIN
      /** Runs as the retry reads the nonce. */
      let onRead: (() => void) | null = null
      const reader = {
        ...w.chain,
        call: async (...a: Parameters<typeof w.chain.call>) => {
          onRead?.()
          return w.chain.call(...a)
        }
      }
      const p = page(w, { login: () => who, services: { reader: () => reader } })
      await p.open()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', retry: true }))
      // Signed out when RETRY is pressed.
      who = null
      p.shown.at(-1)!.handlers.onRetry?.()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', message: 'Sign in again with the same account to retry this purchase.' }))
      expect(w.relay.posts).toHaveLength(0)
      // Back, and signed out again while the nonce is read.
      who = LOGIN
      p.shown.at(-1)!.handlers.onCheckAgain?.()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', retry: true }))
      onRead = () => {
        who = null
      }
      p.shown.at(-1)!.handlers.onRetry?.()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', message: 'Sign in again with the same account to retry this purchase.' }))
      expect(w.relay.posts).toHaveLength(0)
      expect(stored(w, '0000000b-0000-0000-0000-000000000004').mana.sends).toEqual([FIRST])
    })

    it('stops offering RETRY when the record holds as many sends as it keeps, and still checks the chain', async () => {
      const w = world({ allowance: maxUint256 })
      w.clock.now = RETRY_AFTER_MS + 10
      const sends = Array.from({ length: 8 }, (_, i) => ({ id: `s${i}`, stage: 'dispatched', payload: keccak256(SIGNED), at: 0 }))
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
    const SIGNED = ('0x0c53c51c' + 'cd'.repeat(40)) as Hex
    /** A MANA purchase this browser kept mid-way, as a page that went would leave it. */
    const seed = (w: World, id: string, sends: Record<string, unknown>[], signed: Hex = SIGNED) =>
      w.storage.setItem(
        `dcl-inworld-shop:record:mana-v1:${BUYER}:${id}`,
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
            ...(sends.some((x) => x.payload) ? { signed } : {}),
            sends
          },
          createdAt: 1,
          updatedAt: 1
        })
      )

    it('ends a purchase none of whose sends was dispatched as cancelled: it provably sent nothing', async () => {
      const w = world()
      seed(w, '0000000a-0000-0000-0000-000000000001', [{ id: 's4', stage: 'signing' }])
      const p = page(w)
      await p.shell.request('shop.status', {})
      await vi.waitFor(() => expect(p.events.at(-1)).toMatchObject({ operation: '0000000a-0000-0000-0000-000000000001', state: 'cancelled' }))
    })

    it('ends a purchase whose only sent request the relay refused as failed, a retry left signing dropped on takeover', async () => {
      const w = world()
      seed(w, '0000000a-0000-0000-0000-000000000004', [
        { id: 'a1', stage: 'refused', payload: keccak256(SIGNED), at: 1 },
        { id: 'b1', stage: 'signing' }
      ])
      const p = page(w)
      await p.shell.request('shop.status', {})
      await vi.waitFor(() => expect(p.events.filter((e) => e.operation === '0000000a-0000-0000-0000-000000000004').at(-1)?.state).toBe('failed'))
      expect(p.events.filter((e) => e.operation === '0000000a-0000-0000-0000-000000000004').map((e) => e.state)).not.toContain('cancelled')
      const kept = JSON.parse(w.storage.getItem(`dcl-inworld-shop:record:mana-v1:${BUYER}:0000000a-0000-0000-0000-000000000004`)!) as { mana: { sends: { id: string }[] } }
      expect(kept.mana.sends.map((x) => x.id)).toEqual(['a1'])
    })

    it.each([
      ['once, and ends it failed on the next look, without a reload', 1, 'failed'],
      ['every time, and keeps it waiting', Infinity, 'processing']
    ])('when tidying the record on takeover fails %s', async (_what, refusals, end) => {
      const w = world()
      const id = '0000000a-0000-0000-0000-000000000005'
      seed(w, id, [
        { id: 'a1', stage: 'refused', payload: keccak256(SIGNED), at: 1 },
        { id: 'b1', stage: 'signing' }
      ])
      let refused = 0
      // The tidied record (only A) is refused as many times as asked; every other write goes through.
      const storage = w.storage
      const flaky: Storage = {
        get length() {
          return storage.length
        },
        clear: () => storage.clear(),
        getItem: (k) => storage.getItem(k),
        key: (i) => storage.key(i),
        removeItem: (k) => storage.removeItem(k),
        setItem: (k, v) => {
          if (k.endsWith(id) && v.includes('"a1"') && !v.includes('"b1"') && !v.includes('"failed"') && refused < refusals) {
            refused++
            throw new DOMException('full', 'QuotaExceededError')
          }
          storage.setItem(k, v)
        }
      }
      const p = page(w, { storage: flaky, services: { sleep: () => new Promise((r) => setTimeout(r, 20)) } })
      await p.shell.request('shop.status', {})
      await new Promise((r) => setTimeout(r, 150))
      const states = p.events.filter((e) => e.operation === id).map((e) => e.state)
      expect(states).not.toContain('cancelled')
      if (end === 'failed') expect(states.at(-1)).toBe('failed')
      else expect(states.some((x) => x === 'failed')).toBe(false)
      expect(refused).toBeGreaterThan(0)
    })

    it('still reads a sale that went on chain while tidying its record keeps failing', async () => {
      const w = world()
      const id = '0000000a-0000-0000-0000-000000000006'
      const input = ('0x0c53c51c' + 'ab'.repeat(40)) as Hex
      const hash = ('0x' + '9a'.repeat(32)) as Hex
      w.txs.set(hash, { to: STORE, input })
      w.chain.receipts.set(hash, 'success')
      // Landed past the record's checkpoint (900), and final at the first look.
      w.onChain.receiptBlock = 950n
      seed(
        w,
        id,
        [
          { id: 'a1', stage: 'dispatched', payload: keccak256(input), at: 1, hash },
          { id: 'b1', stage: 'signing' }
        ],
        input
      )
      const storage = w.storage
      const stuck: Storage = {
        get length() {
          return storage.length
        },
        clear: () => storage.clear(),
        getItem: (k) => storage.getItem(k),
        key: (i) => storage.key(i),
        removeItem: (k) => storage.removeItem(k),
        setItem: (k, v) => {
          // The tidied record (A without B) never sticks.
          if (k.endsWith(id) && v.includes('"a1"') && !v.includes('"b1"') && !v.includes('"succeeded"')) throw new DOMException('full', 'QuotaExceededError')
          storage.setItem(k, v)
        }
      }
      const p = page(w, { storage: stuck })
      await p.shell.request('shop.status', {})
      await vi.waitFor(() => expect(p.events.filter((e) => e.operation === id).at(-1)).toMatchObject({ state: 'succeeded', txHash: hash }))
    })

    it('keeps a dispatched purchase the chain can’t settle yet, and a BUY of its item shows it', async () => {
      const w = world()
      seed(w, '0000000a-0000-0000-0000-000000000002', [{ id: 's5', stage: 'dispatched', payload: keccak256(SIGNED), at: w.clock.now }])
      const p = page(w)
      const answer = await p.open()
      expect(answer).toMatchObject({ operation: '0000000a-0000-0000-0000-000000000002', state: 'processing' })
      expect(p.shown.at(-1)?.view).toMatchObject({ step: 'pending', message: 'Checking your purchase. You can close this window.' })
      await new Promise((r) => setTimeout(r, 30))
      expect(p.events.some((e) => e.operation === '0000000a-0000-0000-0000-000000000002' && e.state !== 'processing')).toBe(false)
      expect(w.wallet.requests.some((q) => q.method === 'eth_signTypedData_v4' || q.method === 'eth_sendTransaction')).toBe(false)
    })

    it('keeps a MANA purchase under its own key, where a build that doesn’t know MANA never finds it', async () => {
      const w = world({ allowance: maxUint256 })
      w.relay.state.mode = 'down'
      const p = page(w)
      await p.payMana()
      await p.until('pending')
      const op = p.events.at(-1)!.operation
      const keys = [...Array(w.storage.length).keys()].map((i) => w.storage.key(i)!)
      expect(keys).toContain(`dcl-inworld-shop:record:mana-v1:${BUYER}:${op}`)
      expect(keys.some((k) => k.startsWith(`dcl-inworld-shop:record:v1:${BUYER}:`) && k.endsWith(op))).toBe(false)
    })

    it('never keeps a send as gone once a write for the purchase failed: storage holds what this page did', async () => {
      const w = world({ allowance: maxUint256 })
      // The checkout's progress write right before the relay request fails, after the signature was kept. Every record
      // write is watched: a page that died right after any of them must not find a send kept as gone.
      const dispatchedWrites: string[] = []
      const storage = memoryStorage((key, value) => {
        if (key.includes(':record:') && value.includes('"stage":"dispatched"')) dispatchedWrites.push(key)
        return key.includes(':operation:v1:') && value.includes('"submitted"')
      })
      const p = page(w, { storage })
      await p.payMana()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view.step).toBe('failed'))
      expect(w.relay.posts).toHaveLength(0)
      expect(dispatchedWrites).toEqual([])
    })

    it('sends nothing when the purchase’s copy under the Credits key can’t be removed as it becomes a MANA purchase', async () => {
      const w = world({ allowance: maxUint256 })
      const base = w.storage
      const storage: Storage = {
        get length() {
          return base.length
        },
        clear: () => base.clear(),
        getItem: (k) => base.getItem(k),
        key: (i) => base.key(i),
        removeItem: (k) => {
          if (k.startsWith(`dcl-inworld-shop:record:v1:${BUYER}:`)) throw new DOMException('blocked', 'SecurityError')
          base.removeItem(k)
        },
        setItem: (k, v) => base.setItem(k, v)
      }
      const p = page(w, { storage })
      await p.payMana()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view.step).toBe('failed'))
      expect(w.relay.posts).toHaveLength(0)
      expect(signedNonces(w)).toEqual([])
    })

    it('reads a record that moved between the two keys by its latest copy, a sale that went only as MANA, and MANA only under its own key', () => {
      const w = world()
      const id = '0000000a-0000-0000-0000-000000000009'
      seed(w, id, [{ id: 's9', stage: 'signing' }])
      const signing = JSON.parse(w.storage.getItem(`dcl-inworld-shop:record:mana-v1:${BUYER}:${id}`)!) as Record<string, unknown>
      const credits = (updatedAt: number) => JSON.stringify({ ...signing, mana: undefined, updatedAt })
      const ledger = createLedger(() => w.storage)
      // A copy left under the Credits key, older: the MANA one is the purchase.
      w.storage.setItem(`dcl-inworld-shop:record:v1:${BUYER}:${id}`, credits(0))
      expect(ledger.saved(BUYER, id)?.mana).toBeDefined()
      expect(ledger.load(BUYER).filter((r) => r.id === id)).toHaveLength(1)
      // Written later, while nothing of the sale went: that one is.
      w.storage.setItem(`dcl-inworld-shop:record:v1:${BUYER}:${id}`, credits(5))
      expect(ledger.saved(BUYER, id)?.mana).toBeUndefined()
      // Once a send went, the MANA record is the purchase, however late the other copy was written (a cancellation, say).
      seed(w, id, [{ id: 's9', stage: 'dispatched', payload: keccak256(SIGNED), at: 1 }])
      const manaRecord = JSON.parse(w.storage.getItem(`dcl-inworld-shop:record:mana-v1:${BUYER}:${id}`)!) as Record<string, unknown>
      w.storage.setItem(`dcl-inworld-shop:record:v1:${BUYER}:${id}`, JSON.stringify({ ...manaRecord, mana: undefined, state: 'cancelled', updatedAt: 9 }))
      expect(ledger.saved(BUYER, id)).toMatchObject({ state: 'processing', mana: { sends: [{ stage: 'dispatched' }] } })
      // A save moves it, and leaves nothing under the other key.
      expect(ledger.save(manaRecord as unknown as Parameters<typeof ledger.save>[0])).toBe(true)
      expect(w.storage.getItem(`dcl-inworld-shop:record:v1:${BUYER}:${id}`)).toBeNull()
      // A MANA attempt under the Credits key is never read.
      w.storage.removeItem(`dcl-inworld-shop:record:mana-v1:${BUYER}:${id}`)
      w.storage.setItem(`dcl-inworld-shop:record:v1:${BUYER}:${id}`, JSON.stringify(manaRecord))
      expect(ledger.saved(BUYER, id)).toBeNull()
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

  describe('a cart paid in MANA', () => {
    const MARKET = BOOK.marketplace.contract.toLowerCase()
    const BOOTS = '8'
    const HAT = '9'
    const NAMES: Record<string, string> = { [BOOTS]: 'Silver Boots', [HAT]: 'Straw Hat', [String(ITEM)]: 'Golden Armour' }
    /**
     * A cart of a Collection Store mint (BOOTS) and a creator's MANA listing on the Marketplace (ITEM): two sales, one
     * per venue. `items` changes what is in it; an item is a store mint unless it is ITEM.
     */
    const cartPage = (w: World, opts: Parameters<typeof page>[1] & { items?: string[] } = {}) => {
      const listing = manaTrade()
      const items = opts.items ?? [BOOTS, String(ITEM)]
      const p = page(w, {
        trade: () => listing,
        ...opts,
        services: {
          fetchItem: async (_c, itemId) => ({ ...LISTING, id: `listing-${itemId}`, itemId, name: NAMES[itemId] }) as unknown as CatalogItem,
          offers: () => async (queries, session) =>
            Object.fromEntries(
              queries.map((q) => {
                const itemId = q.asset.token.kind === 'item' ? q.asset.token.itemId : ''
                return [q.key, itemId === String(ITEM) ? manaListingOffer(session) : storeMint(session, 'wearable', BigInt(itemId))]
              })
            ),
          ...opts.services
        }
      })
      const openCart = () => p.shell.request('shop.buy', { v: SHOP_PROTOCOL, items: items.map((itemId) => ({ contract: CONTRACT, itemId, quantity: 1 })), method: 'credits', cart: true })
      const payMana = async () => {
        await openCart()
        ;(await p.until('method')).handlers.onPay?.('mana')
      }
      return { ...p, openCart, payMana }
    }
    const allowed = (w: World, ...venues: string[]) => venues.forEach((venue) => w.chain.allowances.set(`137:${BUYER}:${venue}`, maxUint256))
    /** Each step shown with a payment count, in order: "processing 1/2". */
    const counted = (p: ReturnType<typeof page>) => p.shown.flatMap((s) => ('progress' in s.view && s.view.progress ? [`${s.view.step} ${s.view.progress.at}/${s.view.progress.of}`] : []))
    /** Each operation's last state, in the order they started. */
    const ends = (p: ReturnType<typeof page>) => [...new Map(p.events.map((e) => [e.operation, e.state])).values()]
    const relayAt = (w: World, refuse: string) =>
      (async (url: string, init?: RequestInit) => {
        const target = String((JSON.parse(String(init?.body)) as { transactionData: { params: unknown[] } }).transactionData.params[0]).toLowerCase()
        if (target === refuse) Object.assign(w.relay.state, { mode: 'refuse', refuseStatus: 429, refuseBody: { error: 'quota_reached', quota: 30 } })
        return w.relay.fetchImpl(url, init)
      }) as unknown as typeof fetch

    it('offers MANA for all of it at its sales’ total, beside Credits that can’t buy one of its items', async () => {
      const w = world({ advancing: true })
      const p = cartPage(w)
      await p.openCart()
      const { view } = await p.until('method')
      expect(view).toMatchObject({ step: 'method', credits: { price: null, pays: false }, mana: { price: 2n * BigInt(PRICE), balance: 40n * WEI, pays: true } })
    })

    it('shows MANA the buyer holds that can’t pay for all of it, beside Credits that do', async () => {
      const w = world({ advancing: true })
      const p = cartPage(w, { items: [BOOTS, HAT], manaBalance: 3n * WEI })
      await p.openCart()
      const { view } = await p.until('method')
      expect(view).toMatchObject({ credits: { price: 24, pays: true }, mana: { price: 2n * BigInt(PRICE), balance: 3n * WEI, pays: false } })
    })

    it('leaves MANA out of all of it when MANA can’t buy one of its items', async () => {
      const w = world({ advancing: true })
      const two = creatorTrade({
        received: [
          { assetType: ASSET_TYPE.ERC20, contractAddress: BOOK.mana, amount: PRICE, extra: '0x', beneficiary: BOOK.creator },
          { assetType: ASSET_TYPE.ERC20, contractAddress: BOOK.mana, amount: '1', extra: '0x', beneficiary: BOOK.creator }
        ]
      })
      const p = cartPage(w, { trade: () => two })
      await p.openCart()
      await p.until('failed')
      expect(p.manaReads).toEqual([137])
      expect(p.shown.some((x) => x.view.step === 'method')).toBe(false)
    })

    it('buys it one sale per venue, one after another, saying which payment is running', async () => {
      const w = world({ advancing: true })
      allowed(w, STORE, MARKET)
      const listing = manaTrade()
      let reads = 0
      const p = cartPage(w, { trade: () => (reads++, listing) })
      await p.payMana()
      const done = await p.until('success')
      expect(done.view).toMatchObject({ step: 'success', items: [{ name: 'Silver Boots' }, { name: 'Golden Armour' }] })
      expect(relayedTo(w)).toEqual([STORE, MARKET])
      expect([...new Set(counted(p))]).toEqual(['processing 1/2', 'processing 2/2'])
      // Processing stays up between the sales: nothing says it is in progress, closable, while the rest is still to run.
      const second = p.shown.findIndex((s) => s.view.step === 'processing' && s.view.progress?.at === 2)
      expect(p.shown.slice(0, second).some((s) => s.view.step === 'pending')).toBe(false)
      // The listing read for the terms, then once when every sale was reviewed before any ran: never again to run it.
      expect(reads).toBe(2)
      expect(ends(p)).toEqual(['succeeded', 'succeeded'])
      expect(w.wallet.sent).toHaveLength(0)
    })

    it('pays a cart whose items are one sale as one purchase, with no count', async () => {
      const w = world({ advancing: true })
      allowed(w, STORE)
      const p = cartPage(w, { items: [BOOTS, HAT] })
      await p.openCart()
      const method = await p.until('method')
      expect(method.view).toMatchObject({ mana: { price: 2n * BigInt(PRICE), pays: true } })
      method.handlers.onPay?.('mana')
      await p.until('success')
      expect(relayedTo(w)).toEqual([STORE])
      expect(counted(p)).toEqual([])
      expect(ends(p)).toEqual(['succeeded'])
    })

    it('asks each venue for its allowance in its turn, counted', async () => {
      const w = world({ advancing: true })
      const p = cartPage(w)
      await p.payMana()
      const first = await p.until('authorize')
      expect(first.view).toMatchObject({ venue: 'the Collection Store', progress: { at: 1, of: 2 } })
      first.handlers.onAuthorize?.()
      await vi.waitFor(() => expect(p.shown.at(-1)?.view).toMatchObject({ step: 'authorize', venue: 'the Marketplace', state: 'ready', progress: { at: 2, of: 2 } }))
      p.shown.at(-1)!.handlers.onAuthorize?.()
      await p.until('success')
      expect(relayedTo(w)).toEqual([MANA, STORE, MANA, MARKET])
    })

    it('refuses all of it, with nothing signed or sent, when a later sale changed since it was shown', async () => {
      const w = world({ advancing: true })
      allowed(w, STORE, MARKET)
      const shown = manaTrade()
      const other = { ...shown, signature: `0x${'cd'.repeat(65)}` }
      let reads = 0
      const p = cartPage(w, { trade: () => (++reads === 1 ? shown : other) })
      await p.payMana()
      const failed = await p.until('failed')
      expect(failed.view).toMatchObject({ retry: true, message: 'The price or terms of one of these items changed. Try again to see the new ones. No MANA was spent.' })
      expect(w.wallet.requests.some((q) => q.method === 'eth_signTypedData_v4' || q.method === 'eth_sendTransaction')).toBe(false)
      expect(w.relay.posts).toHaveLength(0)
    })

    it('refuses all of it as one purchase when its first sale is refused, with TRY AGAIN', async () => {
      const w = world({ advancing: true })
      allowed(w, STORE, MARKET)
      const p = cartPage(w, { services: { rail: () => ({ relayerUrl: 'https://node.test/transactions-api/v1', fetchImpl: relayAt(w, STORE), gasless: true, wait: { timeoutMs: 30, pollMs: 10 } }) } })
      await p.payMana()
      const failed = await p.until('failed')
      expect(failed.view).toMatchObject({ retry: true })
      expect((failed.view as { message: string }).message).not.toMatch(/^Part of this purchase/)
      expect(relayedTo(w)).toEqual([STORE])
    })

    it('holds the rest while a sale isn’t confirmed, and says what went once the chain settles it', async () => {
      const w = world({ advancing: true })
      allowed(w, STORE, MARKET)
      w.relay.state.confirm = 'none'
      const p = cartPage(w)
      await p.payMana()
      const pending = await p.until('pending')
      // Its step says the rest never went and waits in the cart, while it may take a long while to settle.
      expect(pending.view).toMatchObject({ check: 'ready', message: 'Checking your purchase. You can close this window. The rest of this purchase wasn’t sent while this part is being confirmed, and it’s still in your cart.' })
      expect(relayedTo(w)).toEqual([STORE])
      // The chain shows it now: CHECK AGAIN settles it, and the dialog says what went and what didn't.
      for (const hash of w.txs.keys()) w.chain.receipts.set(hash, 'success')
      pending.handlers.onCheckAgain?.()
      const failed = await p.until('failed')
      expect(failed.view).toMatchObject({
        retry: false,
        message: 'Part of this purchase went through, but not all of it: the rest wasn’t sent while an earlier part was still being confirmed; no MANA was spent for it. What wasn’t bought is still in your cart.'
      })
      expect(relayedTo(w)).toEqual([STORE])
      expect(ends(p)).toEqual(['succeeded'])
    })

    it('says nothing was bought and no MANA spent once the chain finds the sale that held the rest never ran', async () => {
      const w = world({ advancing: true })
      allowed(w, STORE, MARKET)
      w.relay.state.mode = 'down'
      const p = cartPage(w)
      await p.payMana()
      const pending = await p.until('pending')
      // Its nonce went to another call by the buyer, and nothing of ours ran: it can never run.
      w.chain.nonces.set(`${STORE}:${BUYER}`, 1n)
      w.onChain.executions = [{ user: BUYER, call: '0xbeef', txHash: ('0x' + 'cd'.repeat(32)) as Hex, blockNumber: w.onChain.head + 50n, logIndex: 3 }]
      pending.handlers.onCheckAgain?.()
      const failed = await p.until('failed')
      expect(failed.view).toMatchObject({ retry: false, message: 'None of its transactions went through: nothing was bought and no MANA was spent. Everything is still in your cart.' })
      expect(ends(p)).toEqual(['failed'])
    })

    it('says what went when a later sale is refused, and that it spent no MANA', async () => {
      const w = world({ advancing: true })
      allowed(w, STORE, MARKET)
      const p = cartPage(w, { services: { rail: () => ({ relayerUrl: 'https://node.test/transactions-api/v1', fetchImpl: relayAt(w, MARKET), gasless: true, wait: { timeoutMs: 30, pollMs: 10 } }) } })
      await p.payMana()
      const failed = await p.until('failed')
      const message = (failed.view as { message: string }).message
      expect(message).toMatch(/^Part of this purchase went through, but not all of it: /)
      expect(message).toContain("You've used all 30 fee-free transactions this account gets in 24 hours.")
      expect(message).toMatch(/No MANA was spent for it\. What wasn’t bought is still in your cart\.$/)
      expect(failed.view).toMatchObject({ retry: false })
      expect(relayedTo(w)).toEqual([STORE, MARKET])
      expect(ends(p)).toEqual(['succeeded', 'failed'])
    })

    it('runs nothing more when this page can’t hold a later sale’s own operation, keeping what went', async () => {
      const w = world({ advancing: true })
      allowed(w, STORE, MARKET)
      // The first operation's lock, then none for the second (another live tab holds it, say).
      let claims = 0
      const p = cartPage(w, { services: { claim: async () => (++claims === 2 ? null : () => {}) } })
      await p.payMana()
      const failed = await p.until('failed')
      expect(failed.view).toMatchObject({
        retry: false,
        message: 'Part of this purchase went through, but not all of it: this browser couldn’t keep the rest of the purchase to this tab; no MANA was spent for it. What wasn’t bought is still in your cart.'
      })
      expect(relayedTo(w)).toEqual([STORE])
      expect(ends(p)).toEqual(['succeeded', 'cancelled'])
    })

    it('ends the rest when its allowance is cancelled, keeping what went', async () => {
      const w = world({ advancing: true })
      allowed(w, STORE)
      const p = cartPage(w)
      await p.payMana()
      const auth = await p.until('authorize')
      expect(auth.view).toMatchObject({ venue: 'the Marketplace', progress: { at: 2, of: 2 } })
      auth.handlers.onClose()
      const failed = await p.until('failed')
      expect(failed.view).toMatchObject({
        retry: false,
        message: 'Part of this purchase went through, but not all of it: the rest was cancelled before it was paid; no MANA was spent for it. What wasn’t bought is still in your cart.'
      })
      expect(relayedTo(w)).toEqual([STORE])
      expect(ends(p)).toEqual(['succeeded', 'cancelled'])
    })

    it('holds the rest when the relay’s hash for a sale names another transaction, however well its receipt reads', async () => {
      const w = world({ advancing: true })
      allowed(w, STORE, MARKET)
      // A successful receipt for the hash the relay gave, which isn't the sale's signed execution; finality lags.
      w.onChain.foreignHash = true
      w.onChain.frozen = true
      const p = cartPage(w)
      await p.payMana()
      const pending = await p.until('pending')
      expect(relayedTo(w)).toEqual([STORE])
      expect(pending.view).toMatchObject({ message: 'Checking your purchase. You can close this window. The rest of this purchase wasn’t sent while this part is being confirmed, and it’s still in your cart.' })
      expect(ends(p)).toEqual(['pending'])
    })

    it('ends a later sale the relay refused past its deadline through the checkout: what went, and what didn’t', async () => {
      const w = world({ advancing: true })
      allowed(w, STORE, MARKET)
      let release!: () => void
      const held = new Promise<void>((r) => (release = r))
      // The Marketplace's relay request is refused (marked as never sent), and the answer comes after the send's deadline.
      const lateAtMarket = (async (url: string, init?: RequestInit) => {
        const target = String((JSON.parse(String(init?.body)) as { transactionData: { params: unknown[] } }).transactionData.params[0]).toLowerCase()
        if (target !== MARKET) return w.relay.fetchImpl(url, init)
        w.relay.state.mode = 'refuse'
        const res = await w.relay.fetchImpl(url, init)
        await held
        return res
      }) as unknown as typeof fetch
      const sleepers: (() => void)[] = []
      const wake = () => sleepers.splice(0).forEach((r) => r())
      const p = cartPage(w, {
        services: {
          rail: () => ({ relayerUrl: 'https://node.test/transactions-api/v1', fetchImpl: lateAtMarket, gasless: true, wait: { timeoutMs: 30, pollMs: 10 } }),
          sleep: () => new Promise<void>((r) => sleepers.push(r)),
          manaTimeouts: () => ({ sendMs: 50, readMs: 50 })
        }
      })
      await p.payMana()
      await p.until('pending')
      const [, market] = [...new Set(p.events.map((e) => e.operation))]
      await new Promise((r) => setTimeout(r, 120))
      release()
      await new Promise((r) => setTimeout(r, 30))
      wake()
      await vi.waitFor(() => expect(p.events.filter((e) => e.operation === market).at(-1)?.state).toBe('failed'))
      const failed = await p.until('failed')
      expect(failed.view).toMatchObject({
        retry: false,
        message: 'Part of this purchase went through, but not all of it: one of its transactions couldn’t go out; no MANA was spent for it. What wasn’t bought is still in your cart.'
      })
      expect(ends(p)).toEqual(['succeeded', 'failed'])
    })

    it('goes on once the executor sees a sale confirmed, and shows each one still being finalized until the chain settles all of it', async () => {
      const w = world({ advancing: true })
      allowed(w, STORE, MARKET)
      // Both sales confirm while finality lags: the cart runs both, and the chain hasn't settled either yet.
      w.onChain.frozen = true
      const p = cartPage(w)
      await p.payMana()
      const first = await p.until('pending')
      expect(relayedTo(w)).toEqual([STORE, MARKET])
      const [firstOp, secondOp] = [...new Set(p.events.map((e) => e.operation))]
      w.onChain.frozen = false
      first.handlers.onCheckAgain?.()
      await vi.waitFor(() => expect(p.events.filter((e) => e.operation === firstOp).at(-1)).toMatchObject({ state: 'succeeded' }))
      // The one still in progress now: the second, with its own CHECK AGAIN.
      const second = await p.until('pending')
      expect(second).not.toBe(first)
      expect(p.events.filter((e) => e.operation === secondOp).at(-1)).toMatchObject({ state: 'pending' })
      second.handlers.onCheckAgain?.()
      const done = await p.until('success')
      expect(done.view).toMatchObject({ items: [{ name: 'Silver Boots' }, { name: 'Golden Armour' }] })
      expect(ends(p)).toEqual(['succeeded', 'succeeded'])
    })
  })
})

/** The venue's call inside a signed executeMetaTransaction (the CollectionStore's: from, call, r, s, v). */
function decodeCall(payload: Hex): Hex {
  return decodeFunctionData({ abi: EXECUTE_ABI, data: payload }).args[1] as Hex
}
