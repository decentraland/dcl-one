// What may cross into the shell's purchase (lib/shopProtocol) and what the purchase may ask of the wallet
// (shell/shop/walletGuard).
import { describe, expect, it } from 'vitest'
import type { Hex } from 'viem'
import { decodeAbiParameters, encodeAbiParameters, encodeFunctionData, getAbiItem, parseAbi } from 'viem'
import { COLLECTION_STORE_ABI } from '@shop/checkout/chain/abi'
import { metaTxTypedData } from '../../shop/checkout/chain/metatx'
import { createFakeWallet, creatorTrade } from '../../shop/checkout/fakes'
import { buildStoreUseCreditsArgs, buildUseCreditsArgs, encodeUseCredits, type SpendableCredit, type StoreItemToBuy, type UseCreditsArgs } from '../../shop/checkout/purchase/encoding'
import { BOOK, ITEM } from '@shop/payment/fixtures'
import { ADDRESS, OPERATION_STATES, parseBuyNameRequest, parseBuyRequest, parseLines, parseOperationEvent, SHOP_EVENT, SHOP_PROTOCOL } from '../lib/shopProtocol'
import { AMOY_ADD_PARAMS } from '../../shop/wallet/chain'
import { guardedProvider, type PurchaseIntent } from '../shell/shop/walletGuard'

const BUYER = BOOK.buyer.toLowerCase()
const OTHER = '0x' + 'cc'.repeat(20)
const MANAGER = '0x' + 'ce'.repeat(20)
const STORE = BOOK.store.contract.toLowerCase()
const MARKETPLACE = BOOK.marketplace.contract.toLowerCase()
const CHAIN = 137
const STORE_BUY = getAbiItem({ abi: COLLECTION_STORE_ABI, name: 'buy' })
/** A store mint's entries, as encoded. */
const decodeStoreItems = (args: UseCreditsArgs) => [...decodeAbiParameters(STORE_BUY.inputs, args.externalCall.data as Hex)[0]]

describe('the shop.buy request', () => {
  const line = { contract: '0x' + 'AB'.repeat(20), itemId: '7', quantity: 1 }
  const ok = { v: 1, items: [line], method: 'credits' }

  it('admits item references, lowercased, how many of each, and display hints', () => {
    expect(parseBuyRequest(ok)).toEqual({ v: 1, items: [{ contract: '0x' + 'ab'.repeat(20), itemId: '7', quantity: 1 }], method: 'credits' })
    expect(parseBuyRequest({ ...ok, input: 'keyboard' }).input).toBe('keyboard')
    expect(parseBuyRequest({ ...ok, cart: true }).cart).toBe(true)
    const cart = parseBuyRequest({ ...ok, items: [line, { ...line, itemId: '8', quantity: 3 }, { ...line, contract: '0x' + 'cd'.repeat(20) }] })
    expect(cart.items.map((l) => `${l.itemId}x${l.quantity}`)).toEqual(['7x1', '8x3', '7x1'])
    expect(() => parseBuyRequest({ ...ok, input: 'touch' })).toThrow()
  })

  it.each([
    ['another field', { ...ok, buyer: BUYER }],
    ['the app’s name for the creator', { ...ok, creatorName: 'Aurora' }],
    ['a field in an item', { ...ok, items: [{ ...line, tradeId: 'x' }] }],
    ['another version', { ...ok, v: 2 }],
    ['a bad contract', { ...ok, items: [{ ...line, contract: '0x12' }] }],
    ['a bad item id', { ...ok, items: [{ ...line, itemId: '07' }] }],
    ['an item id past uint256', { ...ok, items: [{ ...line, itemId: '1'.repeat(79) }] }],
    ['no items', { ...ok, items: [] }],
    ['an item twice', { ...ok, items: [line, { ...line, contract: line.contract.toLowerCase(), quantity: 2 }] }],
    ['none of an item', { ...ok, items: [{ ...line, quantity: 0 }] }],
    ['part of an item', { ...ok, items: [{ ...line, quantity: 1.5 }] }],
    ['more units than a purchase takes', { ...ok, items: [{ ...line, quantity: 20 }, { ...line, itemId: '8', quantity: 11 }] }],
    ['a cart flag that isn’t true', { ...ok, cart: 'yes' }],
    ['MANA', { ...ok, method: 'mana' }],
    ['nothing', null]
  ])('refuses %s', (_, params) => {
    expect(() => parseBuyRequest(params)).toThrow()
  })

  it('reads only well-formed operation events', () => {
    const event = { type: SHOP_EVENT, v: 1, operation: 'a', buyer: BUYER.toUpperCase().replace('0X', '0x'), state: 'succeeded', items: [line], cart: true, next: 'backpack', txHash: '0xabc' }
    expect(parseOperationEvent(event)).toMatchObject({ buyer: BUYER, state: 'succeeded', items: [{ contract: line.contract.toLowerCase(), itemId: '7', quantity: 1 }], cart: true, next: 'backpack', txHash: '0xabc' })
    expect(parseOperationEvent({ ...event, buyer: null })).toMatchObject({ buyer: null })
    expect(parseOperationEvent({ ...event, buyer: undefined })).toBeNull()
    expect(parseOperationEvent({ ...event, buyer: '0x12' })).toBeNull()
    expect(parseOperationEvent({ ...event, state: 'exploded' })).toBeNull()
    expect(parseOperationEvent({ ...event, v: 2 })).toBeNull()
    expect(parseOperationEvent({ ...event, items: [] })).toBeNull()
  })
})

describe('the shop.buyName request', () => {
  const ok = { v: 1, name: 'Mojito', method: 'credits', mode: 'simulation' }

  it('admits the NAME as typed, the mode the shell said, and how CLAIM NAME was pressed', () => {
    expect(parseBuyNameRequest(ok)).toEqual(ok)
    expect(parseBuyNameRequest({ ...ok, mode: 'live' }).mode).toBe('live')
    expect(parseBuyNameRequest({ ...ok, input: 'pointer' }).input).toBe('pointer')
    expect(parseBuyNameRequest({ ...ok, name: 'ab' }).name).toBe('ab')
    expect(parseBuyNameRequest({ ...ok, name: 'Abcdefghij12345' }).name).toBe('Abcdefghij12345')
  })

  it.each([
    ['a price', { ...ok, price: 1 }],
    ['a buyer', { ...ok, buyer: BUYER }],
    ['a contract', { ...ok, contract: MANAGER }],
    ['items', { ...ok, items: [] }],
    ['a cart', { ...ok, cart: true }],
    ['no mode', { v: 1, name: 'Mojito', method: 'credits' }],
    ['an unknown mode', { ...ok, mode: 'dry-run' }],
    ['one letter', { ...ok, name: 'a' }],
    ['sixteen letters', { ...ok, name: 'Abcdefghij123456' }],
    ['spaces around it', { ...ok, name: ' Mojito ' }],
    ['a space in it', { ...ok, name: 'Moj ito' }],
    ['a letter outside ASCII', { ...ok, name: 'Mojitó' }],
    ['digits outside ASCII', { ...ok, name: 'Name١٢' }],
    ['a dot', { ...ok, name: 'mojito.dcl.eth' }],
    ['a NAME that isn’t text', { ...ok, name: 7 }],
    ['another version', { ...ok, v: 2 }],
    ['MANA', { ...ok, method: 'mana' }],
    ['another input', { ...ok, input: 'touch' }],
    ['nothing', null]
  ])('refuses %s', (_, params) => {
    expect(() => parseBuyNameRequest(params)).toThrow()
  })
})

describe('the operations the HUD reads', () => {
  const line = { contract: '0x' + 'ab'.repeat(20), itemId: '7', quantity: 1 }
  const item = { type: SHOP_EVENT, v: 1, operation: 'a', buyer: BUYER, state: 'succeeded', items: [line] }
  const name = { type: SHOP_EVENT, v: 1, operation: 'n', buyer: BUYER, state: 'succeeded', items: [], name: 'Mojito', mode: 'simulation' }

  it('reads a NAME as its own variant: the NAME, no items, and its mode', () => {
    expect(parseOperationEvent(name)).toEqual({ v: 1, operation: 'n', buyer: BUYER, state: 'succeeded', items: [], name: 'Mojito', mode: 'simulation' })
    expect(parseOperationEvent({ ...name, next: 'previewName' })).toMatchObject({ next: 'previewName' })
    expect(parseOperationEvent({ ...name, mode: 'live', next: 'assignName', txHash: '0xabc' })).toMatchObject({ mode: 'live', next: 'assignName', txHash: '0xabc' })
    expect(parseOperationEvent({ ...name, txHash: null })).toMatchObject({ txHash: null })
    expect(parseOperationEvent({ ...item, next: 'backpack' })).toMatchObject({ next: 'backpack' })
  })

  it.each([
    ['a NAME with items', { ...name, items: [line] }],
    ['a NAME without its empty items', { ...name, items: undefined }],
    ['a NAME without a mode', { ...name, mode: undefined }],
    ['a NAME in an unknown mode', { ...name, mode: 'dry-run' }],
    ['a NAME bought from the cart', { ...name, cart: true }],
    ['a NAME that isn’t one', { ...name, name: 'Moj ito' }],
    ['a NAME too long', { ...name, name: 'Abcdefghij123456' }],
    ['a simulated NAME with a transaction', { ...name, txHash: '0xabc' }],
    ['a simulated NAME assigned to the avatar', { ...name, next: 'assignName' }],
    ['a live NAME previewed', { ...name, mode: 'live', next: 'previewName' }],
    ['a NAME sent to the Backpack', { ...name, next: 'backpack' }],
    ['a NAME previewed before it succeeded', { ...name, state: 'processing', next: 'previewName' }],
    ['a live NAME assigned before it succeeded', { ...name, mode: 'live', state: 'pending', next: 'assignName' }],
    ['items with a mode', { ...item, mode: 'simulation' }],
    ['items previewed as a NAME', { ...item, next: 'previewName' }],
    ['items assigned as a NAME', { ...item, next: 'assignName' }],
    ['items sent to the Backpack before they were bought', { ...item, state: 'pending', next: 'backpack' }],
    ['a cart flag that isn’t true', { ...item, cart: false }],
    ['a transaction that isn’t text', { ...item, txHash: 7 }]
  ])('refuses %s', (_, message) => {
    expect(parseOperationEvent(message as Record<string, unknown>)).toBeNull()
  })

  // The parser a HUD built before NAMEs runs, as it shipped: anything it can't read as items is no operation.
  const shippedBeforeNames = (message: Record<string, unknown>): unknown => {
    if (message.type !== SHOP_EVENT || message.v !== SHOP_PROTOCOL) return null
    const { operation, buyer, state, items } = message
    if (typeof operation !== 'string' || typeof state !== 'string' || !(OPERATION_STATES as readonly string[]).includes(state)) return null
    if (buyer !== null && (typeof buyer !== 'string' || !ADDRESS.test(buyer))) return null
    return parseLines(items) ? message : null
  }

  it('is dropped, NAME by NAME, by a HUD built before NAMEs, which still reads the items’', () => {
    for (const state of OPERATION_STATES) {
      expect(shippedBeforeNames({ ...name, state })).toBeNull()
      expect(shippedBeforeNames({ ...item, state })).not.toBeNull()
    }
    expect(shippedBeforeNames({ ...name, next: 'previewName' })).toBeNull()
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
    units: [{ contract: BOOK.collection.toLowerCase(), itemId: String(ITEM) }],
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

  describe('a cart’s group', () => {
    const other: StoreItemToBuy = { collection: OTHER, itemId: '8', priceWei: '1000000000000000000' }
    const units = [
      { contract: BOOK.collection.toLowerCase(), itemId: String(ITEM) },
      { contract: OTHER, itemId: '8' },
      { contract: OTHER, itemId: '8' }
    ]

    it('signs one mint per confirmed unit, in any order: two of an item are two', async () => {
      const g = setup({ units })
      await expect(sign(g, typed(encodeUseCredits(storeArgs({}, [other, item, other]))))).resolves.toMatch(/^0x/)
      expect(g.forwarded()).toBe(1)
    })

    it.each([
      ['one unit short', [item, other]],
      ['one unit more', [item, other, other, other]],
      ['another unit in place of one', [item, other, { ...other, itemId: '9' }]],
      ['one of an item for two of another', [item, item, other]]
    ])('refuses %s', async (_, items) => {
      const g = setup({ units })
      await expect(sign(g, typed(encodeUseCredits(storeArgs({}, items))))).rejects.toMatchObject({ code: 4100 })
      expect(g.forwarded()).toBe(0)
    })

    it('refuses a unit for someone else among the buyer’s', async () => {
      const g = setup({ units: units.slice(0, 2) })
      const mine = buildStoreUseCreditsArgs(STORE, [item], BUYER, [credit], credit.amount)
      const theirs = buildStoreUseCreditsArgs(STORE, [other], OTHER, [credit], credit.amount)
      const [a] = decodeStoreItems(mine)
      const [b] = decodeStoreItems(theirs)
      const mixed = { ...mine, externalCall: { ...mine.externalCall, data: encodeAbiParameters(STORE_BUY.inputs, [[a, b]]) } }
      await expect(sign(g, typed(encodeUseCredits(mixed)))).rejects.toMatchObject({ code: 4100 })
    })

    it('signs one trade per confirmed unit through the marketplace', async () => {
      const trade = creatorTrade()
      const unit = { contract: BOOK.collection.toLowerCase(), itemId: String(ITEM) }
      const g = setup({ sale: { venue: MARKETPLACE, call: 'accept' }, units: [unit, unit] })
      await expect(sign(g, typed(encodeUseCredits(buildUseCreditsArgs(MARKETPLACE, [trade, trade], BUYER, [credit], credit.amount))))).resolves.toMatch(/^0x/)
      await expect(sign(g, typed(encodeUseCredits(buildUseCreditsArgs(MARKETPLACE, [trade], BUYER, [credit], credit.amount))))).rejects.toMatchObject({ code: 4100 })
    })

    it('never signs for an empty confirmation', async () => {
      const g = setup({ units: [] })
      await expect(sign(g, typed())).rejects.toMatchObject({ code: 4100 })
    })
  })

  it('refuses a listing when the confirmed sale was the store’s mint', async () => {
    const g = setup()
    const args = buildUseCreditsArgs(STORE, [creatorTrade()], BUYER, [credit], credit.amount)
    await expect(sign(g, typed(encodeUseCredits(args)))).rejects.toMatchObject({ code: 4100 })
  })

  it('has the wallet sign the typed data as checked, not the text it was read from', async () => {
    const g = setup()
    // The same data, with a repeated key whose first copy a different parser might keep.
    const data = JSON.stringify(typed())
    const repeated = data.replace('"primaryType":"MetaTransaction"', '"primaryType":"Permit","primaryType":"MetaTransaction"')
    await expect(sign(g, repeated)).resolves.toMatch(/^0x/)
    const forwarded = g.wallet.requests.find((r) => r.method === 'eth_signTypedData_v4')!
    expect((forwarded.params as unknown[])[1]).toBe(JSON.stringify(JSON.parse(repeated)))
    expect((forwarded.params as unknown[])[1]).not.toContain('Permit')
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

  it('switches and adds the network with what this page knows of it, never the caller’s endpoints', async () => {
    const g = setup()
    await g.guarded.request({ method: 'wallet_switchEthereumChain', params: [{ chainId: '0x89', rpcUrls: ['https://rpc.example'] }] })
    expect(g.wallet.requests.at(-1)).toEqual({ method: 'wallet_switchEthereumChain', params: [{ chainId: '0x89' }] })
    // Polygon is every wallet's; only Amoy is ever added, from this page's own parameters.
    await expect(g.guarded.request({ method: 'wallet_addEthereumChain', params: [{ chainId: '0x89' }] })).rejects.toMatchObject({ code: 4100 })
    const amoy = setup({ chainId: 80002 })
    await amoy.guarded.request({ method: 'wallet_addEthereumChain', params: [{ chainId: '0x13882', rpcUrls: ['https://rpc.example'], blockExplorerUrls: ['https://scan.example'] }] }).catch(() => {})
    expect(amoy.wallet.requests.at(-1)).toEqual({ method: 'wallet_addEthereumChain', params: [AMOY_ADD_PARAMS] })
  })

  it('sends directly only while the wallet is on the purchase’s network', async () => {
    const g = setup()
    const tx = { from: BUYER, to: MANAGER, data: encodeUseCredits(storeArgs()) }
    g.wallet.chainId = 1
    await expect(g.guarded.request({ method: 'eth_sendTransaction', params: [tx] })).rejects.toMatchObject({ code: 4100 })
    expect(g.wallet.sent).toEqual([])
    g.wallet.chainId = CHAIN
    await expect(g.guarded.request({ method: 'eth_sendTransaction', params: [tx] })).resolves.toMatch(/^0x/)
  })

  it('signs with exactly the two parameters it asks for', async () => {
    const g = setup()
    await expect(g.guarded.request({ method: 'eth_signTypedData_v4', params: [BUYER, JSON.stringify(typed()), 'extra'] })).rejects.toMatchObject({ code: 4100 })
    expect(g.forwarded()).toBe(0)
  })
})
