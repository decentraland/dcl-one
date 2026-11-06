// The shell purchase on the web Shop's real rails: the executor, its relayed and direct paths, and the
// wallet guard bound to the confirmed purchase. The Shop server, the chain, the relay and the wallet are
// the checkout's fakes; a credit is issued for a dedicated CreditsManager, as the server requires.
import { describe, expect, it, vi } from 'vitest'
import type { Hex } from 'viem'
import { batchAuthorization, createFakeChain, createFakeRelay, createFakeShop, createFakeWallet } from '../../shop/checkout/fakes'
import type { CatalogItem } from '../../shop/discovery/lib/catalogItem'
import type { UsdCents } from '../../shop/model/money'
import { BOOK, ITEM, storeMint } from '../../shop/payment/fixtures'
import type { StoredLogin } from '../features/auth/sso'
import { SHOP_PROTOCOL, type ShopOperationEvent } from '../lib/shopProtocol'
import { createShellShop, type ShellPurchaseHandlers, type ShellPurchaseView } from '../shell/shop/purchase'

const BUYER = BOOK.buyer.toLowerCase()
const CONTRACT = BOOK.collection.toLowerCase()
/** A CreditsManager this deployment controls, not the shared registry one. */
const DEDICATED = '0x' + 'de'.repeat(20)
const REGISTRY = '0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5'

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

function rails(opts: { trusted: string; kind: 'managed' | 'web3'; gasless?: boolean; storage?: Storage }) {
  const chain = createFakeChain()
  const relay = createFakeRelay({ chain, confirm: 'success' })
  const shop = createFakeShop({
    authorizeCreditBatch: ({ body }) => {
      const auth = batchAuthorization((body as { items: { usdPriceCents: number }[] }).items)
      return { ...auth, credit: { ...auth.credit, contract: DEDICATED } }
    },
    cancelCreditAuthorizations: ({ body }) => ({ released: (body as { salts: string[] }).salts.length, retired: 0, spendableAtSeconds: null }),
    reportCreditSubmission: () => ({ recorded: 1 })
  })
  const wallet = createFakeWallet({
    account: BUYER,
    onSend: () => {
      const hash = `0x${'e'.repeat(63)}1` as Hex
      chain.receipts.set(hash, 'success')
      return hash
    }
  })
  const storage = opts.storage ?? memoryStorage()
  const shown: { view: ShellPurchaseView; handlers: ShellPurchaseHandlers }[] = []
  const events: ShopOperationEvent[] = []
  let clock = 0
  const shell = createShellShop({
    login: () => LOGIN,
    wallet: { kind: () => opts.kind, provider: () => wallet },
    ui: { show: (view, handlers) => void shown.push({ view, handlers }), hide: () => {} },
    post: (e) => void events.push(e),
    openExternal: vi.fn(),
    signIn: vi.fn(),
    services: {
      client: () => shop.client,
      fetchItem: async () => LISTING,
      offers: () => async (queries, session) => Object.fromEntries(queries.map((q) => [q.key, storeMint(session)])),
      balance: async () => ({ balanceCents: 7000 as UsdCents, credits: 700 }),
      resolvers: () => ({ trade: async () => null, store: async () => ({ priceWei: '2300000000000000000', available: 10 }) }),
      reader: () => chain,
      primeFlags: async () => {},
      // One tab: every operation's lock is this page's.
      claim: async () => () => {},
      creatorName: async () => null,
      manager: (chainId) => ({ chainId, address: opts.trusted, name: 'Decentraland Credits', version: '1.0.0' }),
      rail: () => ({
        relayerUrl: 'https://node.test/transactions-api/v1',
        fetchImpl: relay.fetchImpl,
        gasless: opts.gasless ?? true,
        wait: { timeoutMs: 30, pollMs: 10, now: () => clock, sleep: async (ms) => void (clock += ms) }
      }),
      storage: () => storage
    }
  })
  const until = async (step: ShellPurchaseView['step']) => {
    await vi.waitFor(() => expect(shown.at(-1)?.view.step).toBe(step), { timeout: 3000 })
    return shown.at(-1)!
  }
  const buy = async () => {
    await shell.request('shop.buy', { v: SHOP_PROTOCOL, items: [{ contract: CONTRACT, itemId: String(ITEM), quantity: 1 }], method: 'credits' })
    ;(await until('confirm')).handlers.onConfirm?.()
  }
  return { shop, chain, relay, wallet, shown, events, until, buy }
}

describe('the shell purchase on the real rails', () => {
  it('relays a managed wallet’s purchase through the dedicated manager the deployment trusts', async () => {
    const r = rails({ trusted: DEDICATED, kind: 'managed' })
    await r.buy()
    await r.until('success')
    const signed = r.wallet.requests.find((q) => q.method === 'eth_signTypedData_v4')!
    const typed = JSON.parse(signed.params![1] as string) as { domain: { verifyingContract: string } }
    expect(typed.domain.verifyingContract.toLowerCase()).toBe(DEDICATED)
    expect(r.relay.posts).toHaveLength(1)
    expect(r.events.at(-1)).toMatchObject({ state: 'succeeded' })
    expect(r.shop.log.some((q) => q.name === 'cancelCreditAuthorizations')).toBe(false)
  })

  it('sends a web3 wallet’s purchase directly to the dedicated manager', async () => {
    const r = rails({ trusted: DEDICATED, kind: 'web3', gasless: false })
    await r.buy()
    await r.until('success')
    expect(r.wallet.sent).toHaveLength(1)
    expect(r.wallet.sent[0].to.toLowerCase()).toBe(DEDICATED)
    expect(r.relay.posts).toHaveLength(0)
  })

  it('refuses a credit issued for a manager the deployment doesn’t trust, and gives the credit back', async () => {
    const r = rails({ trusted: REGISTRY, kind: 'managed' })
    await r.buy()
    const failed = await r.until('failed')
    expect(failed.view).toMatchObject({ step: 'failed', retry: true })
    expect(r.wallet.requests.some((q) => q.method === 'eth_signTypedData_v4')).toBe(false)
    expect(r.relay.posts).toHaveLength(0)
    expect(r.shop.log.find((q) => q.name === 'cancelCreditAuthorizations')?.body).toEqual({ salts: [`0x${'a1'.repeat(32)}`] })
  })

  it('signs nothing when the checkout can’t be kept', async () => {
    const r = rails({ trusted: DEDICATED, kind: 'managed', storage: memoryStorage((key) => key.includes('dcl-shop-checkout')) })
    await r.buy()
    await r.until('failed')
    expect(r.wallet.requests.some((q) => q.method === 'eth_signTypedData_v4')).toBe(false)
    expect(r.relay.posts).toHaveLength(0)
  })

  it('relays nothing when the hand-off isn’t kept: the signed purchase never leaves the browser', async () => {
    // Everything is kept until the record says the purchase was handed off.
    const r = rails({ trusted: DEDICATED, kind: 'managed', storage: memoryStorage((key, value) => key.includes(':record:') && value.includes('"status":"submitted"')) })
    await r.buy()
    await r.until('failed')
    expect(r.wallet.requests.some((q) => q.method === 'eth_signTypedData_v4')).toBe(true)
    expect(r.relay.posts).toHaveLength(0)
  })

  it('sends nothing directly when the hand-off isn’t kept', async () => {
    const r = rails({ trusted: DEDICATED, kind: 'web3', gasless: false, storage: memoryStorage((key, value) => key.includes(':record:') && value.includes('"status":"submitted"')) })
    await r.buy()
    await r.until('failed')
    expect(r.wallet.sent).toHaveLength(0)
  })

  it('refuses to sign once the wallet moved to another account mid-purchase', async () => {
    const r = rails({ trusted: DEDICATED, kind: 'web3', gasless: false })
    const send = r.wallet.request.bind(r.wallet)
    // The account changes right after the purchase reads it, before the transaction is asked for.
    let reads = 0
    r.wallet.request = async (args) => {
      if (args.method === 'eth_accounts' && ++reads === 2) r.wallet.account = '0x' + '7'.repeat(40)
      return send(args)
    }
    await r.buy()
    await r.until('failed')
    expect(r.wallet.sent).toHaveLength(0)
  })
})
