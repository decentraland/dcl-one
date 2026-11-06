// What may cross into the shell's purchase (lib/shopProtocol) and what the purchase may ask of the wallet
// (shell/shop/walletGuard).
import { describe, expect, it } from 'vitest'
import type { Hex } from 'viem'
import { encodeFunctionData, parseAbi } from 'viem'
import { metaTxTypedData } from '../../shop/checkout/chain/metatx'
import { createFakeWallet, creatorTrade } from '../../shop/checkout/fakes'
import { buildStoreUseCreditsArgs, buildUseCreditsArgs, encodeUseCredits, type SpendableCredit, type StoreItemToBuy, type UseCreditsArgs } from '../../shop/checkout/purchase/encoding'
import { BOOK, ITEM } from '../../shop/payment/fixtures'
import { parseBuyRequest, parseOperationEvent, SHOP_EVENT } from '../lib/shopProtocol'
import { guardedProvider, type PurchaseIntent } from '../shell/shop/walletGuard'

const BUYER = BOOK.buyer.toLowerCase()
const OTHER = '0x' + 'cc'.repeat(20)
const MANAGER = '0x' + 'ce'.repeat(20)
const STORE = BOOK.store.contract.toLowerCase()
const MARKETPLACE = BOOK.marketplace.contract.toLowerCase()
const CHAIN = 137

describe('the shop.buy request', () => {
  const ok = { v: 1, item: { contract: '0x' + 'AB'.repeat(20), itemId: '7' }, method: 'credits' }

  it('admits an item reference, lowercased, and a cleaned display name', () => {
    expect(parseBuyRequest({ ...ok, creatorName: ' Aurora\u0007 ' })).toEqual({ v: 1, item: { contract: '0x' + 'ab'.repeat(20), itemId: '7' }, method: 'credits', creatorName: 'Aurora' })
    expect(parseBuyRequest({ ...ok, creatorName: 'x'.repeat(200) }).creatorName).toHaveLength(64)
    expect(parseBuyRequest({ ...ok, input: 'keyboard' }).input).toBe('keyboard')
    expect(() => parseBuyRequest({ ...ok, input: 'touch' })).toThrow()
  })

  it.each([
    ['another field', { ...ok, buyer: BUYER }],
    ['a field in the item', { ...ok, item: { ...ok.item, tradeId: 'x' } }],
    ['another version', { ...ok, v: 2 }],
    ['a bad contract', { ...ok, item: { contract: '0x12', itemId: '7' } }],
    ['a bad item id', { ...ok, item: { contract: ok.item.contract, itemId: '07' } }],
    ['an item id past uint256', { ...ok, item: { contract: ok.item.contract, itemId: '1'.repeat(79) } }],
    ['MANA', { ...ok, method: 'mana' }],
    ['nothing', null]
  ])('refuses %s', (_, params) => {
    expect(() => parseBuyRequest(params)).toThrow()
  })

  it('reads only well-formed operation events', () => {
    const event = { type: SHOP_EVENT, v: 1, operation: 'a', state: 'succeeded', item: { contract: '0x1', itemId: '7' }, next: 'backpack', txHash: '0xabc' }
    expect(parseOperationEvent(event)).toMatchObject({ state: 'succeeded', next: 'backpack', txHash: '0xabc' })
    expect(parseOperationEvent({ ...event, state: 'exploded' })).toBeNull()
    expect(parseOperationEvent({ ...event, v: 2 })).toBeNull()
  })
})

describe('the purchase wallet', () => {
  const credit: SpendableCredit = { id: '0x' + 'a1'.repeat(32), amount: '3000000000000000000', availableAmount: '3000000000000000000', expiresAt: Math.floor(Date.now() / 1000) + 900, signature: '0x' + 'cc'.repeat(65), contract: MANAGER }
  const item: StoreItemToBuy = { collection: BOOK.collection, itemId: String(ITEM), priceWei: '2300000000000000000' }
  const storeArgs = (over: Partial<UseCreditsArgs> = {}, items = [item], buyer = BUYER): UseCreditsArgs => ({ ...buildStoreUseCreditsArgs(STORE, items, buyer, [credit], credit.amount), ...over })
  const intent = (over: Partial<PurchaseIntent> = {}): PurchaseIntent => ({
    buyer: BUYER,
    chainId: CHAIN,
    manager: { chainId: CHAIN, address: MANAGER, name: 'Decentraland Credits', version: '1.0.0' },
    item: { contract: BOOK.collection.toLowerCase(), itemId: String(ITEM) },
    sale: { venue: STORE, call: 'storeBuy' },
    current: () => true,
    ...over
  })
  const typed = (functionData: Hex = encodeUseCredits(storeArgs()), from = BUYER) =>
    metaTxTypedData({ address: MANAGER as Hex, chainId: CHAIN, name: 'Decentraland Credits', version: '1.0.0', metaTx: 'functionData' } as never, from, 0n, functionData)
  function setup(over: Partial<PurchaseIntent> = {}) {
    const wallet = createFakeWallet({ account: BUYER })
    return { wallet, guarded: guardedProvider(wallet, intent(over)), forwarded: () => wallet.requests.filter((r) => r.method === 'eth_signTypedData_v4' || r.method === 'eth_sendTransaction').length }
  }
  const sign = (g: ReturnType<typeof setup>, data: unknown, from: string = BUYER) => g.guarded.request({ method: 'eth_signTypedData_v4', params: [from, typeof data === 'string' ? data : JSON.stringify(data)] })

  it('signs the confirmed store mint: the manager’s meta-transaction, one credit, the item for the buyer', async () => {
    const g = setup()
    await expect(sign(g, typed())).resolves.toMatch(/^0x/)
    expect(g.forwarded()).toBe(1)
  })

  it('signs the confirmed creator listing through its marketplace', async () => {
    const g = setup({ sale: { venue: MARKETPLACE, call: 'accept' } })
    const args = buildUseCreditsArgs(MARKETPLACE, [creatorTrade()], BUYER, [credit], credit.amount)
    await expect(sign(g, typed(encodeUseCredits(args)))).resolves.toMatch(/^0x/)
  })

  const domainCases: [string, (t: ReturnType<typeof typed>) => unknown][] = [
    ['another domain name', (t) => ({ ...t, domain: { ...t.domain, name: 'Decentraland MANA' } })],
    ['another version', (t) => ({ ...t, domain: { ...t.domain, version: '2' } })],
    ['another manager', (t) => ({ ...t, domain: { ...t.domain, verifyingContract: OTHER } })],
    ['another chain', (t) => ({ ...t, domain: { ...t.domain, salt: '0x' + '0'.repeat(63) + '1' } })],
    ['a chainId beside the salt', (t) => ({ ...t, domain: { ...t.domain, chainId: 1 } })],
    ['no declared types', (t) => ({ ...t, types: {} })],
    ['a changed MetaTransaction type', (t) => ({ ...t, types: { ...t.types, MetaTransaction: [{ name: 'nonce', type: 'uint256' }, { name: 'from', type: 'address' }] } })],
    ['another primary type', (t) => ({ ...t, primaryType: 'Permit' })],
    ['an extra message field', (t) => ({ ...t, message: { ...t.message, value: '1' } })],
    ['a message from someone else', (t) => ({ ...t, message: { ...t.message, from: OTHER } })]
  ]
  it.each(domainCases)('refuses %s', async (_, change) => {
    const g = setup()
    await expect(sign(g, change(typed()))).rejects.toMatchObject({ code: 4100 })
    expect(g.forwarded()).toBe(0)
  })

  const callCases: [string, Hex][] = [
    ['a call other than useCredits', encodeFunctionData({ abi: parseAbi(['function revokeCredits(bytes32[] hashes)']), functionName: 'revokeCredits', args: [[]] })],
    ['two credits', encodeUseCredits({ ...storeArgs(), credits: [...storeArgs().credits, ...storeArgs().credits], creditsSignatures: [credit.signature as Hex, credit.signature as Hex] })],
    ['MANA from the wallet', encodeUseCredits(storeArgs({ maxUncreditedValue: 1n }))],
    ['more than the credit holds', encodeUseCredits(storeArgs({ maxCreditedValue: 4n * 10n ** 18n }))],
    ['a custom call', encodeUseCredits(storeArgs({ customExternalCallSignature: '0x1234' }))],
    ['another venue', encodeUseCredits({ ...storeArgs(), externalCall: { ...storeArgs().externalCall, target: OTHER as Hex } })],
    ['another item', encodeUseCredits(storeArgs({}, [{ ...item, itemId: '8' }]))],
    ['another collection', encodeUseCredits(storeArgs({}, [{ ...item, collection: OTHER }]))],
    ['someone else as the beneficiary', encodeUseCredits(storeArgs({}, [item], OTHER))],
    ['two items', encodeUseCredits(storeArgs({}, [item, item]))],
    ['a call valid for years', encodeUseCredits({ ...storeArgs(), externalCall: { ...storeArgs().externalCall, expiresAt: BigInt(Math.floor(Date.now() / 1000) + 365 * 86400) } })],
    ['calldata it can’t decode', '0xdeadbeef']
  ]
  it.each(callCases)('refuses to sign %s', async (_, data) => {
    const g = setup()
    await expect(sign(g, typed(data))).rejects.toMatchObject({ code: 4100 })
    expect(g.forwarded()).toBe(0)
  })

  it('refuses a listing when the confirmed sale was the store’s mint', async () => {
    const g = setup()
    const args = buildUseCreditsArgs(STORE, [creatorTrade()], BUYER, [credit], credit.amount)
    await expect(sign(g, typed(encodeUseCredits(args)))).rejects.toMatchObject({ code: 4100 })
  })

  it('reads the wallet’s account before signing, and refuses another one', async () => {
    const g = setup()
    g.wallet.account = OTHER
    await expect(sign(g, typed())).rejects.toMatchObject({ code: 4100 })
    expect(g.forwarded()).toBe(0)
    expect(g.wallet.requests.some((r) => r.method === 'eth_accounts')).toBe(true)
  })

  it('checks the login again after the wallet answers with the account, and forwards nothing once it changed', async () => {
    let valid = true
    const g = setup({ current: () => valid })
    const answer = g.wallet.request.bind(g.wallet)
    g.wallet.request = async (args) => {
      // A logout (or a renewal of the same account's login) while the wallet reads the account.
      if (args.method === 'eth_accounts') valid = false
      return answer(args)
    }
    await expect(sign(g, typed())).rejects.toMatchObject({ code: 4100 })
    expect(g.forwarded()).toBe(0)
  })

  it('forwards nothing until the purchase is kept for the step', async () => {
    const stages: string[] = []
    const g = setup({ durable: (stage) => (stages.push(stage), false) })
    await expect(sign(g, typed())).rejects.toMatchObject({ code: 4100 })
    await expect(g.guarded.request({ method: 'eth_sendTransaction', params: [{ from: BUYER, to: MANAGER, data: encodeUseCredits(storeArgs()) }] })).rejects.toMatchObject({ code: 4100 })
    expect(stages).toEqual(['authorized', 'handedOff'])
    expect(g.forwarded()).toBe(0)
  })

  it('refuses once the confirmed login is no longer current', async () => {
    const g = setup({ current: () => false })
    await expect(sign(g, typed())).rejects.toMatchObject({ code: 4100 })
    expect(g.forwarded()).toBe(0)
  })

  it('sends the same call directly: to the manager, from the buyer, with no value or unknown fields', async () => {
    const g = setup()
    const data = encodeUseCredits(storeArgs())
    await expect(g.guarded.request({ method: 'eth_sendTransaction', params: [{ from: BUYER, to: MANAGER, data, maxFeePerGas: '0x1' }] })).resolves.toMatch(/^0x/)
    for (const tx of [{ from: OTHER, to: MANAGER, data }, { from: BUYER, to: OTHER, data }, { from: BUYER, to: MANAGER, data, value: '0x1' }, { from: BUYER, to: MANAGER, data, nonce: '0x1' }, { from: BUYER, to: MANAGER, data: encodeUseCredits(storeArgs({ maxUncreditedValue: 1n })) }]) {
      await expect(g.guarded.request({ method: 'eth_sendTransaction', params: [tx] })).rejects.toMatchObject({ code: 4100 })
    }
    expect(g.forwarded()).toBe(1)
  })

  it('answers the account only when the wallet is on the login’s', async () => {
    expect(await setup().guarded.request({ method: 'eth_requestAccounts' })).toEqual([BUYER])
    const g = setup()
    g.wallet.account = OTHER
    await expect(g.guarded.request({ method: 'eth_accounts' })).rejects.toMatchObject({ code: 4100 })
  })

  it('switches only to its own network, and uses nothing else', async () => {
    const g = setup()
    await expect(g.guarded.request({ method: 'wallet_switchEthereumChain', params: [{ chainId: '0x89' }] })).resolves.toBeNull()
    await expect(g.guarded.request({ method: 'wallet_switchEthereumChain', params: [{ chainId: '0x1' }] })).rejects.toMatchObject({ code: 4100 })
    for (const method of ['personal_sign', 'eth_sign', 'eth_signTypedData', 'wallet_watchAsset']) {
      await expect(g.guarded.request({ method, params: [] })).rejects.toMatchObject({ code: 4200 })
    }
  })
})
