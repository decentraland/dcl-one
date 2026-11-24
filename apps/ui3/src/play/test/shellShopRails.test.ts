// The shell purchase on the web Shop's real rails: the executor, its relayed and direct paths, and the
// wallet guard bound to the confirmed purchase. The Shop server, the chain, the relay and the wallet are
// the checkout's fakes; a credit is issued for a dedicated CreditsManager, as the server requires.
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Hex } from 'viem'
import { batchAuthorization, createFakeChain, createFakeRelay, createFakeShop, createFakeWallet } from '../../shop/checkout/fakes'
import type { CatalogItem } from '@shop/discovery/lib/catalogItem'
import type { UsdCents } from '@shop/model/money'
import { BOOK, ITEM, storeMint } from '@shop/payment/fixtures'
import type { StoredLogin } from '@explorer-core/features/auth/sso'
import { SHOP_PROTOCOL, type ShopOperationEvent } from '@dcl/one-ui/apps/shop/shopProtocol'
import { createShellShop, FEE_BALANCE_TIMEOUT_MS, type ShellPurchaseHandlers, type ShellPurchaseView } from '../shell/shop/purchase'

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

afterEach(() => {
  vi.unstubAllEnvs()
})

function rails(opts: { trusted: string; kind: 'managed' | 'web3'; gasless?: boolean; storage?: Storage; walletChain?: number }) {
  // The deployment names its manager the way a build does, so the checkout's address book (which the rails
  // spend through) and the shell's trust agree on it.
  vi.stubEnv('VITE_SHOP_CHAIN_ID', '137')
  vi.stubEnv('VITE_SHOP_CREDITS_MANAGER', opts.trusted)
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
    ...(opts.walletChain != null ? { chainId: opts.walletChain } : {}),
    account: BUYER,
    onSend: (tx) => {
      const hash = `0x${'e'.repeat(63)}1` as Hex
      chain.receipts.set(hash, 'success')
      // The chain holds the wallet's own transaction too, so a purchase can be found out by its call.
      chain.transactions.set(hash.toLowerCase(), { from: BUYER, to: tx.to, input: tx.data })
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
      // What the chain says a hash's transaction is: the relay's and the wallet's, as the fake chain holds them.
      manaEvidence: () => ({ transaction: async (c: number, h: string) => (await chain.transaction?.(c as never, h)) ?? null, receipt: (c: number, h: string) => chain.receipt(c as never, h), finalizedTime: async () => 0 }) as never,
      primeFlags: async () => {},
      // One tab: every operation's lock is this page's.
      claim: async () => () => {},
      creatorName: async () => null,
      creatorFace: async () => null,
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
  return { shell, shop, chain, relay, wallet, shown, events, until, buy }
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

  it('holds a purchase the wallet sent and then dropped before its hash: nothing given back, nothing sent again', async () => {
    const r = rails({ trusted: DEDICATED, kind: 'web3', gasless: false })
    r.wallet.next = 'lost'
    await r.buy()
    await r.until('pending')
    expect(r.wallet.sent).toHaveLength(1)
    expect(r.shop.log.some((q) => q.name === 'cancelCreditAuthorizations')).toBe(false)
    expect(r.shown.some((s) => s.view.step === 'failed')).toBe(false)
    expect(r.events.some((e) => e.state === 'failed' || e.state === 'succeeded')).toBe(false)
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

  // Answers after a send, without the relay's evidence that nothing was sent: this relay's 504 (the
  // node took it but its answer was lost, or answered an error to the send), and the 500 JSON an
  // earlier version of this relay, or a transactions-server, answers after it sent.
  it.each([
    ['managed', 504],
    ['web3', 504],
    ['managed', 500],
    ['web3', 500]
  ] as const)('follows a %s wallet’s purchase the relay may have sent (%i), and never signs a second one for it', async (kind, failStatus) => {
    const r = rails({ trusted: DEDICATED, kind })
    r.relay.state.mode = 'fail-after-send'
    r.relay.state.failStatus = failStatus
    await r.buy()
    const pending = await r.until('pending')
    expect(r.events.at(-1)).toMatchObject({ state: 'pending' })
    // No TRY AGAIN, no cancel, no second send from the wallet.
    expect(r.shown.some((s) => s.view.step === 'failed')).toBe(false)
    expect(r.shop.log.some((q) => q.name === 'cancelCreditAuthorizations')).toBe(false)
    expect(r.wallet.sent).toHaveLength(0)
    pending.handlers.onClose()
    // Buying the same item again shows that purchase: the same operation, the same credit.
    const first = r.events.at(-1)!.operation
    const again = await r.shell.request('shop.buy', { v: SHOP_PROTOCOL, items: [{ contract: CONTRACT, itemId: String(ITEM), quantity: 1 }], method: 'credits' })
    expect(again).toMatchObject({ operation: first, state: 'pending' })
    expect(r.shown.at(-1)?.view.step).toBe('pending')
    expect(r.shop.log.filter((q) => q.name === 'authorizeCreditBatch')).toHaveLength(1)
    expect(r.wallet.requests.filter((q) => q.method === 'eth_signTypedData_v4')).toHaveLength(1)
    expect(r.relay.posts).toHaveLength(1)
    expect(r.wallet.sent).toHaveLength(0)
  })

  it('still treats a refusal the relay marks as not sent as one', async () => {
    // The relay's 400 with `"not_sent": true`. A managed wallet: the purchase fails with TRY AGAIN and
    // gives its credit back.
    const managed = rails({ trusted: DEDICATED, kind: 'managed' })
    managed.relay.state.mode = 'refuse'
    await managed.buy()
    expect((await managed.until('failed')).view).toMatchObject({ step: 'failed', retry: true })
    expect(managed.shop.log.find((q) => q.name === 'cancelCreditAuthorizations')?.body).toEqual({ salts: [`0x${'a1'.repeat(32)}`] })
    // A web3 wallet: it sends the same credit itself.
    const web3 = rails({ trusted: DEDICATED, kind: 'web3' })
    web3.relay.state.mode = 'refuse'
    await web3.buy()
    await web3.until('success')
    expect(web3.relay.posts).toHaveLength(1)
    expect(web3.wallet.sent).toHaveLength(1)
    expect(web3.shop.log.filter((q) => q.name === 'authorizeCreditBatch')).toHaveLength(1)
  })

  it('offers a funded web3 wallet on another network the switch, which moves it and buys on a fresh attempt', async () => {
    // The relay refuses (not sent); the wallet's own transaction needs Polygon, and the wallet is on Ethereum.
    const r = rails({ trusted: DEDICATED, kind: 'web3', walletChain: 1 })
    r.relay.state.mode = 'refuse'
    await r.buy()
    const failed = await r.until('failed')
    expect(failed.view).toMatchObject({ step: 'failed', retry: true, switchTo: 'Polygon' })
    expect((failed.view as { message: string }).message).toBe('Fee-free checkout is unavailable right now. You can switch to Polygon and pay the network fee with your wallet. Nothing was bought.')
    expect(r.wallet.sent).toHaveLength(0)
    expect(r.shop.log.find((q) => q.name === 'cancelCreditAuthorizations')?.body).toEqual({ salts: [`0x${'a1'.repeat(32)}`] })
    // One click: the wallet moves to Polygon, then a fresh attempt reads the terms anew.
    failed.handlers.onSwitchNetwork?.()
    failed.handlers.onSwitchNetwork?.() // a second press while the wallet answers does nothing more
    ;(await r.until('confirm')).handlers.onConfirm?.()
    await r.until('success')
    expect(r.wallet.requests.filter((q) => q.method === 'wallet_switchEthereumChain')).toEqual([{ method: 'wallet_switchEthereumChain', params: [{ chainId: '0x89' }] }])
    expect(r.wallet.chainId).toBe(137)
    expect(r.wallet.sent).toHaveLength(1)
    expect(r.shop.log.filter((q) => q.name === 'authorizeCreditBatch')).toHaveLength(2)
    expect(r.events.at(-1)).toMatchObject({ state: 'succeeded' })
  })

  it('tells a web3 wallet without the fee money that it didn’t go out, with no talk of networks', async () => {
    const r = rails({ trusted: DEDICATED, kind: 'web3', walletChain: 1 })
    r.relay.state.mode = 'refuse'
    r.chain.balance = async () => 10n ** 16n // 0.01, under the 0.05 floor
    await r.buy()
    const failed = await r.until('failed')
    expect(failed.view).toMatchObject({ step: 'failed', retry: true, message: 'Fee-free checkout is unavailable right now. Nothing was bought. Try again later.' })
    expect(failed.view).not.toHaveProperty('switchTo')
    expect(r.wallet.requests.some((q) => q.method === 'wallet_switchEthereumChain')).toBe(false)
  })

  it('leaves the switch on offer when the buyer declines it in the wallet, and says so when the switch fails', async () => {
    const r = rails({ trusted: DEDICATED, kind: 'web3', walletChain: 1 })
    r.relay.state.mode = 'refuse'
    const ask = r.wallet.request.bind(r.wallet)
    let answer: 'decline' | 'fail' = 'decline'
    let switches = 0
    r.wallet.request = async (args) => {
      if (args.method === 'wallet_switchEthereumChain') {
        switches++
        throw answer === 'decline' ? Object.assign(new Error('User rejected the request.'), { code: 4001 }) : Object.assign(new Error('Internal error'), { code: -32603 })
      }
      return ask(args)
    }
    await r.buy()
    const failed = await r.until('failed')
    const shown = r.shown.length
    failed.handlers.onSwitchNetwork?.()
    await vi.waitFor(() => expect(switches).toBe(1))
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(r.shown).toHaveLength(shown)
    // Declined, the offer stands: pressing again asks the wallet again, and this time the switch fails.
    answer = 'fail'
    failed.handlers.onSwitchNetwork?.()
    await vi.waitFor(() => expect(r.shown.length).toBeGreaterThan(shown))
    expect(switches).toBe(2)
    expect(r.shown.at(-1)!.view).toMatchObject({ step: 'failed', retry: true, message: 'Your wallet didn’t move to Polygon. Switch it there yourself, then try again.' })
    expect(r.wallet.sent).toHaveLength(0)
  })

  it('keeps the offer when the wallet answers the switch but stays on its network, until it really moves', async () => {
    // A wallet may add a network and leave the one it was on selected (EIP-3085): its answer isn't a switch.
    const r = rails({ trusted: DEDICATED, kind: 'web3', walletChain: 1 })
    r.relay.state.mode = 'refuse'
    const ask = r.wallet.request.bind(r.wallet)
    let stays = true
    r.wallet.request = async (args) => {
      if (args.method === 'wallet_switchEthereumChain' && stays) return null
      return ask(args)
    }
    await r.buy()
    const failed = await r.until('failed')
    const shown = r.shown.length
    failed.handlers.onSwitchNetwork?.()
    await vi.waitFor(() => expect(r.wallet.requests.filter((q) => q.method === 'eth_chainId').length).toBeGreaterThan(0))
    await new Promise((resolve) => setTimeout(resolve, 0))
    // No fresh checkout while the wallet is still on Ethereum.
    expect(r.shown).toHaveLength(shown)
    expect(r.wallet.chainId).toBe(1)
    expect(r.shop.log.filter((q) => q.name === 'authorizeCreditBatch')).toHaveLength(1)
    // The offer stands: this press really moves it, and the purchase starts again.
    stays = false
    failed.handlers.onSwitchNetwork?.()
    ;(await r.until('confirm')).handlers.onConfirm?.()
    await r.until('success')
    expect(r.wallet.chainId).toBe(137)
    expect(r.shop.log.filter((q) => q.name === 'authorizeCreditBatch')).toHaveLength(2)
  })

  it('never offers a managed wallet on another network the switch, nor reads its balance', async () => {
    const r = rails({ trusted: DEDICATED, kind: 'managed', gasless: false, walletChain: 1 })
    let reads = 0
    r.chain.balance = async () => {
      reads++
      return 10n ** 18n
    }
    await r.buy()
    const failed = await r.until('failed')
    expect(failed.view).toMatchObject({ step: 'failed', retry: true, message: 'This purchase couldn’t go out. Nothing was bought. Try again later.' })
    expect(failed.view).not.toHaveProperty('switchTo')
    expect(reads).toBe(0)
  })

  it('offers a funded web3 wallet the switch plainly on a checkout that only sends from the wallet', async () => {
    const r = rails({ trusted: DEDICATED, kind: 'web3', gasless: false, walletChain: 1 })
    await r.buy()
    const failed = await r.until('failed')
    expect(failed.view).toMatchObject({
      step: 'failed',
      switchTo: 'Polygon',
      message: 'Your wallet is on another network. Switch it to Polygon to send this purchase. Nothing was bought.'
    })
    expect(r.relay.posts).toHaveLength(0)
  })

  it('leaves the offer out when the balance never answers, and the failure can still be closed or tried again', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
    try {
      const r = rails({ trusted: DEDICATED, kind: 'web3', walletChain: 1 })
      r.relay.state.mode = 'refuse'
      let asked = false
      r.chain.balance = () => {
        asked = true
        return new Promise<bigint>(() => {})
      }
      await r.buy()
      await vi.waitFor(() => expect(asked).toBe(true))
      expect(r.shown.at(-1)!.view.step).toBe('processing')
      await vi.advanceTimersByTimeAsync(FEE_BALANCE_TIMEOUT_MS)
      const failed = await r.until('failed')
      expect(failed.view).toMatchObject({ step: 'failed', retry: true, message: 'Fee-free checkout is unavailable right now. Nothing was bought. Try again later.' })
      expect(failed.view).not.toHaveProperty('switchTo')
      expect(failed.handlers.onRetry).toBeTypeOf('function')
      expect(failed.handlers.onClose).toBeTypeOf('function')
      expect(r.shop.log.filter((q) => q.name === 'authorizeCreditBatch')).toHaveLength(1)
      expect(r.wallet.sent).toHaveLength(0)
    } finally {
      vi.useRealTimers()
    }
  })

  it('says the account’s spent fee-free quota, and when it frees up, to a managed wallet', async () => {
    const r = rails({ trusted: DEDICATED, kind: 'managed' })
    r.relay.state.mode = 'refuse'
    r.relay.state.refuseStatus = 429
    r.relay.state.refuseBody = { error: 'quota_reached', message: 'Max amount of transactions reached for address. Quota: 30', quota: 30 }
    await r.buy()
    const failed = await r.until('failed')
    expect(failed.view).toMatchObject({
      step: 'failed',
      retry: true,
      message: "You've used all 30 fee-free transactions this account gets in 24 hours. Try again later."
    })
    expect(failed.view).not.toHaveProperty('switchTo')
  })

  it('gives a funded web3 wallet the relay’s reason beside the switch', async () => {
    const r = rails({ trusted: DEDICATED, kind: 'web3', walletChain: 1 })
    r.relay.state.mode = 'refuse'
    r.relay.state.refuseStatus = 429
    r.relay.state.refuseBody = { error: 'relay_budget_spent', message: 'The relay budget for today is spent' }
    await r.buy()
    const failed = await r.until('failed')
    expect(failed.view).toMatchObject({
      step: 'failed',
      switchTo: 'Polygon',
      message: 'The shared fee-free limit for this operation has been reached. Try again later. You can switch to Polygon and pay the network fee with your wallet. Nothing was bought.'
    })
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
