// The MANA wallet guard (shell/shop/manaGuard), bound to one confirmed in-world purchase paid in MANA: the payload
// checks as vectors (services/catalyst/tests/fixtures/shop/mana-purchase-guard-vectors.json, built here with the rails'
// own encoders and held to exactly what is built; `UPDATE_MANA_PURCHASE_GUARD_VECTORS=1` rewrites it), and the
// provider's flow (the session, the chain, the sale's hand-off, the snapshot it sends).

import { readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { afterAll, describe, expect, it, vi } from 'vitest'
import { encodeFunctionData, maxUint256, type Hex } from 'viem'
import { ERC20_ABI } from '@shop/checkout/chain/abi'
import { BOOK, CREATOR_COUPON_ADDRESS, ITEM } from '@shop/payment/fixtures'
import { metaTxTypedData } from '../../shop/checkout/chain/metatx'
import { creatorTrade } from '../../shop/checkout/fakes'
import { getCouponManagerForTrade } from '../../shop/checkout/lib/marketplace'
import { ASSET_TYPE, encodeMarketplaceCall, encodeStoreBuy, type ListingCoupon, type StoreItemToBuy } from '../../shop/checkout/purchase/encoding'
import { checkManaTerms, checkMetaTransaction, checkTransaction, manaGuardedProvider, type ManaHandOff, type ManaPurchaseIntent, type ManaSaleTerms } from '../shell/shop/manaGuard'
import { WalletRefusal } from '../shell/shop/walletGuard'

const FILE = join(__dirname, '../../../../../services/catalyst/tests/fixtures/shop/mana-purchase-guard-vectors.json')
const NOW_MS = 1_767_225_600_000
const CHAIN = 137
const BUYER = BOOK.buyer.toLowerCase()
const OTHER = '0x' + 'cc'.repeat(20)
const MANA = BOOK.mana.toLowerCase()
const MARKETPLACE = BOOK.marketplace.contract.toLowerCase()
const STORE = BOOK.store.contract.toLowerCase()
const COLLECTION = BOOK.collection.toLowerCase()
const PRICE = 2_300_000_000_000_000_000n

const MANA_DOMAIN = { address: MANA as Hex, chainId: CHAIN, name: '(PoS) Decentraland MANA', version: '1', metaTx: 'functionSignature' as const }
const MARKET_DOMAIN = { address: MARKETPLACE as Hex, chainId: CHAIN, name: 'DecentralandMarketplacePolygon', version: '1.0.0', metaTx: 'functionData' as const }
const STORE_DOMAIN = { address: STORE as Hex, chainId: CHAIN, name: 'Decentraland Collection Store', version: '1', metaTx: 'functionSignature' as const }

/** A creator's listing priced in MANA itself. */
const manaListing = (overrides: Parameters<typeof creatorTrade>[0] = {}) =>
  creatorTrade({ received: [{ assetType: ASSET_TYPE.ERC20, contractAddress: BOOK.mana, amount: PRICE.toString(), extra: '0x', beneficiary: BOOK.creator }], ...overrides })
const coupon = (over: Partial<ListingCoupon> = {}): ListingCoupon =>
  ({
    id: 'sale-1',
    signer: BOOK.creator,
    couponManager: getCouponManagerForTrade(manaListing())!,
    couponAddress: CREATOR_COUPON_ADDRESS,
    checks: { uses: 10, expiration: NOW_MS + 86_400_000, effective: NOW_MS - 1_000, salt: '0x00', contractSignatureIndex: 0, signerSignatureIndex: 0, allowedRoot: '0x', externalChecks: [] },
    discountType: 1,
    discount: 100_000,
    root: '0x' + '00'.repeat(32),
    signature: '0x' + 'bb'.repeat(65),
    proof: [],
    ...over
  }) as unknown as ListingCoupon
const ONE_UNIT = [{ contract: COLLECTION, itemId: String(ITEM) }]
const item: StoreItemToBuy = { collection: BOOK.collection, itemId: String(ITEM), priceWei: PRICE.toString() }

const listings = (calldata: Hex = encodeMarketplaceCall([manaListing()], BUYER), over: Partial<Extract<ManaSaleTerms, { kind: 'listings' }>> = {}): ManaSaleTerms => ({
  kind: 'listings',
  venue: MARKETPLACE,
  calldata,
  units: ONE_UNIT,
  ...over
})
const mints = (maxWei = PRICE, over: Partial<Extract<ManaSaleTerms, { kind: 'mints' }>> = {}): ManaSaleTerms => ({ kind: 'mints', venue: STORE, units: ONE_UNIT, maxWei, ...over })
const approve = (spender = MARKETPLACE, amount = maxUint256) => encodeFunctionData({ abi: ERC20_ABI, functionName: 'approve', args: [spender as Hex, amount] })
const typed = (domain: typeof MANA_DOMAIN | typeof MARKET_DOMAIN | typeof STORE_DOMAIN, data: Hex, from = BUYER, nonce = 0n) =>
  JSON.parse(JSON.stringify(metaTxTypedData(domain as never, from, nonce, data))) as Record<string, Record<string, unknown>>

// ---- Vectors ------------------------------------------------------------------------------------

type IntentData = { buyer: string; chainId: number; sale: Record<string, unknown> }
type Vector =
  | { name: string; kind: 'metaTransaction'; intent: IntentData; from: string; data: unknown; expect: 'sign' | 'refuse'; refusal?: string }
  | { name: string; kind: 'transaction'; intent: IntentData; tx: Record<string, unknown>; expect: 'send' | 'refuse'; refusal?: string }

/** Terms as data: a total in wei as a decimal string. */
const asData = (sale: ManaSaleTerms): Record<string, unknown> => (sale.kind === 'mints' ? { ...sale, maxWei: sale.maxWei.toString() } : { ...sale })
const fromData = (sale: Record<string, unknown>): ManaSaleTerms =>
  (sale.kind === 'mints' ? { ...sale, maxWei: BigInt(sale.maxWei as string) } : sale) as unknown as ManaSaleTerms

function buildVectors(): Vector[] {
  const intent = (sale: ManaSaleTerms, over: Partial<IntentData> = {}): IntentData => ({ buyer: BUYER, chainId: CHAIN, sale: asData(sale), ...over })
  const meta = (name: string, sale: ManaSaleTerms, data: unknown, expect: 'sign' | 'refuse', over: Partial<IntentData> = {}): Vector => ({ name, kind: 'metaTransaction', intent: intent(sale, over), from: BUYER, data, expect })
  const send = (name: string, sale: ManaSaleTerms, tx: Record<string, unknown>, expect: 'send' | 'refuse'): Vector => ({ name, kind: 'transaction', intent: intent(sale), tx, expect })
  const withCoupon = encodeMarketplaceCall([manaListing()], BUYER, [coupon()])
  const twoUnits = [ONE_UNIT[0], ONE_UNIT[0]]
  return [
    // Successes: both meta-transaction styles, and direct sends.
    meta('approve MANA to the marketplace (functionSignature)', listings(), typed(MANA_DOMAIN, approve()), 'sign'),
    meta('approve MANA to the CollectionStore (functionSignature)', mints(), typed(MANA_DOMAIN, approve(STORE)), 'sign'),
    meta('accept the confirmed MANA listing (functionData)', listings(), typed(MARKET_DOMAIN, encodeMarketplaceCall([manaListing()], BUYER)), 'sign'),
    meta('accept the confirmed listing with its coupon (functionData)', listings(withCoupon), typed(MARKET_DOMAIN, withCoupon), 'sign'),
    meta('buy the confirmed mint at the confirmed total (functionSignature)', mints(), typed(STORE_DOMAIN, encodeStoreBuy([item], BUYER)), 'sign'),
    meta('two units of a reusable listing, as confirmed', listings(encodeMarketplaceCall([manaListing(), manaListing()], BUYER), { units: twoUnits }), typed(MARKET_DOMAIN, encodeMarketplaceCall([manaListing(), manaListing()], BUYER)), 'sign'),
    send('approve MANA from the wallet', listings(), { from: BUYER, to: MANA, data: approve() }, 'send'),
    send('accept the confirmed listing from the wallet', listings(), { from: BUYER, to: MARKETPLACE, data: encodeMarketplaceCall([manaListing()], BUYER), value: '0x0' }, 'send'),
    send('buy the confirmed mint from the wallet', mints(), { from: BUYER, to: STORE, data: encodeStoreBuy([item], BUYER) }, 'send'),
    // The cap, at the wei.
    meta('a mint one wei over the confirmed total', mints(PRICE - 1n), typed(STORE_DOMAIN, encodeStoreBuy([item], BUYER)), 'refuse'),
    // The listing's own terms.
    meta('a changed coupon discount under the same seller signature', listings(withCoupon), typed(MARKET_DOMAIN, encodeMarketplaceCall([manaListing()], BUYER, [coupon({ discount: 900_000 } as Partial<ListingCoupon>)])), 'refuse'),
    meta('a changed coupon proof (callerData) under the same seller signature', listings(withCoupon), typed(MARKET_DOMAIN, encodeMarketplaceCall([manaListing()], BUYER, [coupon({ proof: ['0x' + '11'.repeat(32)] } as Partial<ListingCoupon>)])), 'refuse'),
    meta('the listing without its confirmed coupon', listings(withCoupon), typed(MARKET_DOMAIN, encodeMarketplaceCall([manaListing()], BUYER)), 'refuse'),
    meta('a listing paying another price under the same seller signature', listings(), typed(MARKET_DOMAIN, encodeMarketplaceCall([manaListing({ received: [{ assetType: ASSET_TYPE.ERC20, contractAddress: BOOK.mana, amount: (PRICE * 10n).toString(), extra: '0x', beneficiary: BOOK.creator }] })], BUYER)), 'refuse'),
    meta('one unit more than confirmed', listings(), typed(MARKET_DOMAIN, encodeMarketplaceCall([manaListing(), manaListing()], BUYER)), 'refuse'),
    // Token, venue, recipient.
    meta('approve another spender', listings(), typed(MANA_DOMAIN, approve(OTHER)), 'refuse'),
    meta('approve a finite amount', listings(), typed(MANA_DOMAIN, approve(MARKETPLACE, PRICE)), 'refuse'),
    meta('approve the CollectionStore for a listing', listings(), typed(MANA_DOMAIN, approve(STORE)), 'refuse'),
    send('approve on another token', listings(), { from: BUYER, to: OTHER, data: approve() }, 'refuse'),
    meta('the sale under another contract’s domain', listings(), typed({ ...MARKET_DOMAIN, address: OTHER as Hex }, encodeMarketplaceCall([manaListing()], BUYER)), 'refuse'),
    meta('a mint for another account', mints(), typed(STORE_DOMAIN, encodeStoreBuy([item], OTHER)), 'refuse'),
    meta('a listing sent to another account', listings(), typed(MARKET_DOMAIN, encodeMarketplaceCall([manaListing()], OTHER)), 'refuse'),
    send('a sale with value', listings(), { from: BUYER, to: MARKETPLACE, data: encodeMarketplaceCall([manaListing()], BUYER), value: '0x1' }, 'refuse'),
    send('a sale with a malformed value', listings(), { from: BUYER, to: MARKETPLACE, data: encodeMarketplaceCall([manaListing()], BUYER), value: 'zero' }, 'refuse'),
    send('a sale from another account', listings(), { from: OTHER, to: MARKETPLACE, data: encodeMarketplaceCall([manaListing()], BUYER) }, 'refuse'),
    meta('a USD-priced listing', listings(encodeMarketplaceCall([creatorTrade()], BUYER)), typed(MARKET_DOMAIN, encodeMarketplaceCall([creatorTrade()], BUYER)), 'refuse'),
    meta('a venue the chain’s registry doesn’t list', listings(encodeMarketplaceCall([manaListing()], BUYER), { venue: OTHER }), typed(MARKET_DOMAIN, encodeMarketplaceCall([manaListing()], BUYER)), 'refuse')
  ].map((v) => withOutcome(v))
}

/** A vector with the outcome the guard gives it now, and the refusal's words. */
function withOutcome(v: Vector): Vector {
  try {
    if (v.kind === 'metaTransaction') checkMetaTransaction(v.from, v.data, { ...v.intent, sale: fromData(v.intent.sale) })
    else checkTransaction(v.tx, { ...v.intent, sale: fromData(v.intent.sale) })
    return v
  } catch (e) {
    if (!(e instanceof WalletRefusal)) throw e
    return { ...v, expect: 'refuse', refusal: e.message }
  }
}

describe('the MANA guard’s vectors', () => {
  // Every vector is built at one fixed clock: the listing fixtures' checks count from now.
  vi.useFakeTimers({ toFake: ['Date'] })
  vi.setSystemTime(new Date(NOW_MS))
  afterAll(() => void vi.useRealTimers())
  const built = buildVectors()
  if (process.env.UPDATE_MANA_PURCHASE_GUARD_VECTORS === '1') writeFileSync(FILE, `${JSON.stringify(built, null, 2)}\n`)
  const committed = JSON.parse(readFileSync(FILE, 'utf8')) as Vector[]

  it('are exactly what the rails build and the guard decides now', () => {
    expect(committed).toEqual(built)
  })

  it.each(built.map((v) => [v.name, v] as const))('%s', (_name, v) => {
    const intent = { ...v.intent, sale: fromData(v.intent.sale) }
    const run = () => (v.kind === 'metaTransaction' ? checkMetaTransaction(v.from, v.data, intent) : checkTransaction(v.tx, intent))
    if (v.expect === 'refuse') expect(run).toThrow(WalletRefusal)
    else expect(run).not.toThrow()
  })

  it('refuses every case the review names, and signs every success', () => {
    const outcome = Object.fromEntries(built.map((v) => [v.name, v.expect]))
    for (const name of ['a mint one wei over the confirmed total', 'a changed coupon discount under the same seller signature', 'a changed coupon proof (callerData) under the same seller signature', 'approve another spender', 'approve on another token', 'a mint for another account', 'a USD-priced listing']) {
      expect(outcome[name], name).toBe('refuse')
    }
    expect(outcome['buy the confirmed mint at the confirmed total (functionSignature)']).toBe('sign')
    expect(outcome['accept the confirmed MANA listing (functionData)']).toBe('sign')
    expect(outcome['accept the confirmed listing from the wallet']).toBe('send')
  })
})

// ---- The provider -------------------------------------------------------------------------------

/** A wallet on `chain` with the buyer's account; `requests` logs what reached it. */
function wallet(chain = CHAIN) {
  const requests: { method: string; params?: unknown[] }[] = []
  let account = BUYER
  return {
    requests,
    setAccount: (a: string) => void (account = a),
    request: async ({ method, params }: { method: string; params?: unknown[] }) => {
      requests.push({ method, params })
      if (method === 'eth_accounts' || method === 'eth_requestAccounts') return [account]
      if (method === 'eth_chainId') return `0x${chain.toString(16)}`
      if (method === 'eth_signTypedData_v4') return '0x' + '22'.repeat(65)
      if (method === 'eth_sendTransaction') return '0x' + 'ab'.repeat(32)
      return null
    }
  }
}

function guarded(sale: ManaSaleTerms, opts: { chain?: number; chainId?: number; handOff?: (attempt: ManaHandOff) => boolean; current?: () => boolean; mayApprove?: () => boolean } = {}) {
  const w = wallet(opts.chain ?? opts.chainId)
  const handOff = vi.fn(opts.handOff ?? (() => true))
  const intent: ManaPurchaseIntent = { buyer: BUYER, chainId: opts.chainId ?? CHAIN, sale, current: opts.current ?? (() => true), handOff, mayApprove: opts.mayApprove ?? (() => true) }
  return { w, handOff, intent, provider: manaGuardedProvider(w, intent) }
}
const reached = (w: ReturnType<typeof wallet>, method: string) => w.requests.filter((r) => r.method === method)

describe('the MANA guard’s provider', () => {
  it('refuses terms it can’t bound before asking the wallet anything: a USD-priced listing, an unlisted venue', () => {
    expect(() => guarded(listings(encodeMarketplaceCall([creatorTrade()], BUYER)))).toThrow(WalletRefusal)
    expect(() => guarded(listings(undefined, { venue: OTHER }))).toThrow(WalletRefusal)
    expect(() => checkManaTerms({ buyer: BUYER, chainId: 1, sale: listings() })).toThrow(WalletRefusal)
  })

  it('signs the approve without a hand-off, and the sale only after one, naming the relay and the nonce it signs', async () => {
    const g = guarded(listings())
    await g.provider.request({ method: 'eth_signTypedData_v4', params: [BUYER, JSON.stringify(typed(MANA_DOMAIN, approve()))] })
    expect(g.handOff).not.toHaveBeenCalled()
    await g.provider.request({ method: 'eth_signTypedData_v4', params: [BUYER, JSON.stringify(typed(MARKET_DOMAIN, encodeMarketplaceCall([manaListing()], BUYER), BUYER, 41n))] })
    expect(g.handOff).toHaveBeenCalledTimes(1)
    expect(g.handOff).toHaveBeenCalledWith({ rail: 'relay', nonce: 41n })
    expect(reached(g.w, 'eth_signTypedData_v4')).toHaveLength(2)
  })

  it('forwards MANA’s approve only while the host says the buyer is authorizing it; the sale needs no such word', async () => {
    let authorizing = false
    const g = guarded(listings(), { mayApprove: () => authorizing })
    await expect(g.provider.request({ method: 'eth_signTypedData_v4', params: [BUYER, JSON.stringify(typed(MANA_DOMAIN, approve()))] })).rejects.toThrow(WalletRefusal)
    await expect(g.provider.request({ method: 'eth_sendTransaction', params: [{ from: BUYER, to: MANA, data: approve() }] })).rejects.toThrow(WalletRefusal)
    expect(reached(g.w, 'eth_signTypedData_v4')).toHaveLength(0)
    expect(reached(g.w, 'eth_sendTransaction')).toHaveLength(0)
    authorizing = true
    await g.provider.request({ method: 'eth_signTypedData_v4', params: [BUYER, JSON.stringify(typed(MANA_DOMAIN, approve()))] })
    expect(reached(g.w, 'eth_signTypedData_v4')).toHaveLength(1)
    authorizing = false
    await g.provider.request({ method: 'eth_signTypedData_v4', params: [BUYER, JSON.stringify(typed(MARKET_DOMAIN, encodeMarketplaceCall([manaListing()], BUYER)))] })
    expect(reached(g.w, 'eth_signTypedData_v4')).toHaveLength(2)
  })

  it('refuses a listing paid in more than one entry, even when the first is the price', () => {
    const two = manaListing({
      received: [
        { assetType: ASSET_TYPE.ERC20, contractAddress: BOOK.mana, amount: PRICE.toString(), extra: '0x', beneficiary: BOOK.creator },
        { assetType: ASSET_TYPE.ERC20, contractAddress: BOOK.mana, amount: '1', extra: '0x', beneficiary: BOOK.creator }
      ]
    })
    expect(() => checkManaTerms({ buyer: BUYER, chainId: CHAIN, sale: listings(encodeMarketplaceCall([two], BUYER)) })).toThrow(WalletRefusal)
  })

  it('hands a sale sent from the wallet off as direct, its nonce the wallet’s to pick', async () => {
    const g = guarded(listings())
    await g.provider.request({ method: 'eth_sendTransaction', params: [{ from: BUYER, to: MANA, data: approve() }] })
    expect(g.handOff).not.toHaveBeenCalled()
    await g.provider.request({ method: 'eth_sendTransaction', params: [{ from: BUYER, to: MARKETPLACE, data: encodeMarketplaceCall([manaListing()], BUYER) }] })
    expect(g.handOff).toHaveBeenCalledTimes(1)
    expect(g.handOff).toHaveBeenCalledWith({ rail: 'direct' })
  })

  it('takes a sale with no approve at all, when an allowance stands already', async () => {
    const g = guarded(mints())
    await g.provider.request({ method: 'eth_signTypedData_v4', params: [BUYER, JSON.stringify(typed(STORE_DOMAIN, encodeStoreBuy([item], BUYER)))] })
    expect(g.handOff).toHaveBeenCalledTimes(1)
    expect(reached(g.w, 'eth_signTypedData_v4')).toHaveLength(1)
  })

  it('never reaches the wallet with a sale the host can’t keep, or that is already on its way', async () => {
    const g = guarded(listings(), { handOff: () => false })
    await expect(g.provider.request({ method: 'eth_signTypedData_v4', params: [BUYER, JSON.stringify(typed(MARKET_DOMAIN, encodeMarketplaceCall([manaListing()], BUYER)))] })).rejects.toThrow(WalletRefusal)
    await expect(g.provider.request({ method: 'eth_sendTransaction', params: [{ from: BUYER, to: MARKETPLACE, data: encodeMarketplaceCall([manaListing()], BUYER) }] })).rejects.toThrow(WalletRefusal)
    expect(reached(g.w, 'eth_signTypedData_v4')).toHaveLength(0)
    expect(reached(g.w, 'eth_sendTransaction')).toHaveLength(0)
    // The approve doesn't need a hand-off, so it still goes.
    await g.provider.request({ method: 'eth_signTypedData_v4', params: [BUYER, JSON.stringify(typed(MANA_DOMAIN, approve()))] })
    expect(reached(g.w, 'eth_signTypedData_v4')).toHaveLength(1)
  })

  it('sends the checked snapshot, whatever the caller does to its own object while the wallet is read', async () => {
    const g = guarded(listings())
    const tx: Record<string, unknown> = { from: BUYER, to: MARKETPLACE, data: encodeMarketplaceCall([manaListing()], BUYER), value: '0x0' }
    const original = g.w.request
    g.w.request = async (args) => {
      // The caller swaps the data while the guard awaits the account.
      if (args.method === 'eth_accounts') tx.data = encodeMarketplaceCall([manaListing()], OTHER)
      return original(args)
    }
    await g.provider.request({ method: 'eth_sendTransaction', params: [tx] })
    const [sent] = reached(g.w, 'eth_sendTransaction')
    expect(sent!.params).toEqual([{ from: BUYER, to: MARKETPLACE, data: encodeMarketplaceCall([manaListing()], BUYER), chainId: '0x89' }])
  })

  it('holds the terms as confirmed when the guard was made, whatever is done to the intent after', async () => {
    const g = guarded(mints(PRICE, { units: ONE_UNIT.map((u) => ({ ...u })) }))
    // The caller raises the cap and swaps the units after the guard was made.
    ;(g.intent.sale as Extract<ManaSaleTerms, { kind: 'mints' }>).maxWei = PRICE * 100n
    ;(g.intent.sale as Extract<ManaSaleTerms, { kind: 'mints' }>).units[0]!.contract = OTHER
    const dearer: StoreItemToBuy = { ...item, priceWei: (PRICE * 50n).toString() }
    await expect(g.provider.request({ method: 'eth_signTypedData_v4', params: [BUYER, JSON.stringify(typed(STORE_DOMAIN, encodeStoreBuy([dearer], BUYER)))] })).rejects.toThrow(/more MANA/)
    await g.provider.request({ method: 'eth_signTypedData_v4', params: [BUYER, JSON.stringify(typed(STORE_DOMAIN, encodeStoreBuy([item], BUYER)))] })
    expect(reached(g.w, 'eth_signTypedData_v4')).toHaveLength(1)
  })

  it('signs what it checked for the pinned buyer, whatever the caller changes in its request while the wallet is read', async () => {
    const g = guarded(listings())
    const data = typed(MANA_DOMAIN, approve())
    const params: unknown[] = [BUYER, data]
    const original = g.w.request
    g.w.request = async (args) => {
      if (args.method === 'eth_accounts') {
        // The caller turns the approve toward another spender and names another account.
        ;(data.message as Record<string, unknown>).functionSignature = approve(OTHER)
        params[0] = OTHER
      }
      return original(args)
    }
    await g.provider.request({ method: 'eth_signTypedData_v4', params })
    const [signed] = reached(g.w, 'eth_signTypedData_v4')
    expect(signed!.params![0]).toBe(BUYER)
    expect(JSON.parse(signed!.params![1] as string)).toEqual(typed(MANA_DOMAIN, approve()))
  })

  it('sends the rails’ own fees: none named on Polygon, Amoy’s priority fee there, and refuses a request naming others', async () => {
    const polygon = guarded(listings())
    await polygon.provider.request({ method: 'eth_sendTransaction', params: [{ from: BUYER, to: MANA, data: approve() }] })
    expect(reached(polygon.w, 'eth_sendTransaction')[0]!.params).toEqual([{ from: BUYER, to: MANA, data: approve(), chainId: '0x89' }])
    for (const fees of [{ maxFeePerGas: '0x1' }, { maxPriorityFeePerGas: '0x3b9aca00' }, { gas: '0x5208' }, { maxFeePerGas: 'lots' }]) {
      await expect(polygon.provider.request({ method: 'eth_sendTransaction', params: [{ from: BUYER, to: MANA, data: approve(), ...fees }] }), JSON.stringify(fees)).rejects.toThrow(WalletRefusal)
    }

    const AMOY = 80002
    const AMOY_MANA = '0x7ad72b9f944ea9793cf4055d88f81138cc2c63a0'
    const AMOY_STORE = '0xe36abc9ec616c83caaa386541380829106149d68'
    const policy = { maxPriorityFeePerGas: '0x6fc23ac00', maxFeePerGas: '0x12a05f2000' }
    const amoy = guarded(mints(PRICE, { venue: AMOY_STORE }), { chainId: AMOY })
    await amoy.provider.request({ method: 'eth_sendTransaction', params: [{ from: BUYER, to: AMOY_MANA, data: approve(AMOY_STORE) }] })
    await amoy.provider.request({ method: 'eth_sendTransaction', params: [{ from: BUYER, to: AMOY_STORE, data: encodeStoreBuy([item], BUYER), ...policy }] })
    expect(reached(amoy.w, 'eth_sendTransaction').map((r) => r.params)).toEqual([
      [{ from: BUYER, to: AMOY_MANA, data: approve(AMOY_STORE), chainId: '0x13882', ...policy }],
      [{ from: BUYER, to: AMOY_STORE, data: encodeStoreBuy([item], BUYER), chainId: '0x13882', ...policy }]
    ])
    await expect(amoy.provider.request({ method: 'eth_sendTransaction', params: [{ from: BUYER, to: AMOY_STORE, data: encodeStoreBuy([item], BUYER), maxFeePerGas: '0x1' }] })).rejects.toThrow(WalletRefusal)
    // The same fees written another way are the policy's, and what is sent is the policy's own form.
    await amoy.provider.request({ method: 'eth_sendTransaction', params: [{ from: BUYER, to: AMOY_MANA, data: approve(AMOY_STORE), maxFeePerGas: '80000000000', maxPriorityFeePerGas: 30_000_000_000 }] })
    expect(reached(amoy.w, 'eth_sendTransaction').at(-1)!.params).toEqual([{ from: BUYER, to: AMOY_MANA, data: approve(AMOY_STORE), chainId: '0x13882', ...policy }])
  })

  it('reads the chain last: a wallet that moves to another network while its account is read gets no hand-off and no send', async () => {
    const g = guarded(listings())
    let chain = CHAIN
    const original = g.w.request
    g.w.request = async (args) => {
      if (args.method === 'eth_accounts') chain = 1
      if (args.method === 'eth_chainId') return `0x${chain.toString(16)}`
      return original(args)
    }
    await expect(g.provider.request({ method: 'eth_sendTransaction', params: [{ from: BUYER, to: MARKETPLACE, data: encodeMarketplaceCall([manaListing()], BUYER) }] })).rejects.toThrow(/another network/)
    expect(g.handOff).not.toHaveBeenCalled()
    expect(reached(g.w, 'eth_sendTransaction')).toHaveLength(0)
  })

  it('buys with MANA on Polygon and Amoy only: a listing valid on a NAMEs chain’s registry is refused', () => {
    const ETHEREUM_MARKETPLACE = '0x0f11d0d1671519683bd48abf3dbe779e300941cd'
    expect(() => guarded(listings(undefined, { venue: ETHEREUM_MARKETPLACE }), { chainId: 1 })).toThrow(/doesn’t buy with MANA on this network/)
  })

  it('refuses a direct send on another network, and anything once the login or the wallet’s account changed', async () => {
    const elsewhere = guarded(listings(), { chain: 1 })
    await expect(elsewhere.provider.request({ method: 'eth_sendTransaction', params: [{ from: BUYER, to: MANA, data: approve() }] })).rejects.toThrow(/another network/)
    expect(reached(elsewhere.w, 'eth_sendTransaction')).toHaveLength(0)

    let current = true
    const stale = guarded(listings(), { current: () => current })
    current = false
    await expect(stale.provider.request({ method: 'eth_signTypedData_v4', params: [BUYER, JSON.stringify(typed(MANA_DOMAIN, approve()))] })).rejects.toThrow(/sign-in changed/)

    const moved = guarded(listings())
    moved.w.setAccount(OTHER)
    await expect(moved.provider.request({ method: 'eth_signTypedData_v4', params: [BUYER, JSON.stringify(typed(MANA_DOMAIN, approve()))] })).rejects.toThrow(WalletRefusal)
    expect(reached(moved.w, 'eth_signTypedData_v4')).toHaveLength(0)
    expect(moved.handOff).not.toHaveBeenCalled()
  })

  it('refuses typed data it can’t serialize as its own refusal, before any hand-off or wallet call', async () => {
    const g = guarded(listings())
    const data = { ...typed(MANA_DOMAIN, approve()), extra: 1n }
    await expect(g.provider.request({ method: 'eth_signTypedData_v4', params: [BUYER, data] })).rejects.toMatchObject({ name: 'WalletRefusal', code: 4100 })
    expect(g.handOff).not.toHaveBeenCalled()
    expect(g.w.requests).toHaveLength(0)
  })

  it('switches only to its own network, and refuses any other method', async () => {
    const g = guarded(listings())
    await g.provider.request({ method: 'wallet_switchEthereumChain', params: [{ chainId: '0x89' }] })
    await expect(g.provider.request({ method: 'wallet_switchEthereumChain', params: [{ chainId: '0x1' }] })).rejects.toThrow(WalletRefusal)
    await expect(g.provider.request({ method: 'personal_sign', params: ['0x00', BUYER] })).rejects.toMatchObject({ code: 4200 })
  })
})
