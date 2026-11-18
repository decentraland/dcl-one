import { describe, expect, it, vi } from 'vitest'
import { Network, TradeAssetType, TradeType, type Trade } from '@dcl/schemas'
import { ContractName, getContract } from '~/lib/chain/contracts'
import { zeroAddress } from 'viem'
import { TEST_SIGNER, signTypedDataWithTestKey, toTransaction } from '~/test/chain'
import { TradeConflictError, TradeNotFoundError, tradeSigner } from './trades'
import { type Collection } from './collections'
import { ItemType, type Item } from './items'
import { WrongAccountError, WrongChainError } from './walletErrors'
import {
  NO_EXPIRATION,
  SellItemError,
  buildCancelListingCall,
  buildEnableSalesCall,
  buildItemOrder,
  getListingTerms,
  removeListing,
  removeStoreListing,
  updatePrice,
  withConflictRetry,
  creditsToUsdWei,
  MAX_SALE_CREDITS,
  MAX_SALE_MANA_WEI,
  MIN_SALE_MANA_WEI,
  formatCreditsAsUsd,
  formatManaAsUsd,
  isSamePrice,
  isValidCredits,
  isValidManaWei,
  listingToSalePrice,
  parseManaAmount,
  sanitizeManaInput,
  toPricedSale,
  formatDateValue,
  isSalesEnabled,
  isSellingOnOlderMarketplace,
  isValidAddress,
  minExpirationDate,
  parseExpirationDate,
  sellItem,
  toSellItemError,
  withSalesEnabled,
  type SellItemDeps,
  type SellItemParams
} from './sales'

const CHAIN_ID = 80002
const ADDRESS = '0x00000000000000000000000000000000000000aa'
const BENEFICIARY = '0x00000000000000000000000000000000000000bb'
const CONTRACT = '0x00000000000000000000000000000000000000cc'
const MARKETPLACE_V3 = getContract(ContractName.OffChainMarketplaceV3, CHAIN_ID).address
const MARKETPLACE_V2 = getContract(ContractName.OffChainMarketplaceV2, CHAIN_ID).address
const MARKETPLACE_V1 = getContract(ContractName.OffChainMarketplace, CHAIN_ID).address
const MANA = getContract(ContractName.MANAToken, CHAIN_ID).address
const SALT = '0x' + '11'.repeat(32)

const collection: Collection = {
  id: 'c1',
  name: 'Pirate Hats',
  owner: ADDRESS,
  urn: `urn:decentraland:amoy:collections-v2:${CONTRACT}`,
  contractAddress: CONTRACT,
  isPublished: true,
  isApproved: true,
  itemCount: 1,
  minters: [],
  managers: [],
  createdAt: 1,
  updatedAt: 1
}

const item: Item = {
  id: 'i1',
  name: 'Pirate Hat',
  description: 'Yarr',
  thumbnail: 'thumbnail.png',
  owner: ADDRESS,
  collectionId: 'c1',
  rarity: 'legendary',
  totalSupply: 10,
  tokenId: '3',
  isPublished: true,
  isApproved: true,
  inCatalyst: true,
  type: ItemType.WEARABLE,
  data: { category: 'hat', representations: [] },
  contents: {},
  createdAt: 1,
  updatedAt: 1
}

const params: SellItemParams = {
  address: ADDRESS,
  chainId: CHAIN_ID,
  collection,
  item,
  price: { kind: 'credits', credits: 50 },
  beneficiary: BENEFICIARY,
  expiresAt: NO_EXPIRATION
}
const indexes = { contractSignatureIndex: 1, signerSignatureIndex: 2 }

describe('price and date helpers', () => {
  it('converts whole credits to USD wei at ten cents a credit', () => {
    expect(creditsToUsdWei(1)).toBe('100000000000000000')
    expect(creditsToUsdWei(50)).toBe('5000000000000000000')
    expect(formatCreditsAsUsd(50)).toBe('$5.00')
    expect(formatCreditsAsUsd(1)).toBe('$0.10')
    expect(() => creditsToUsdWei(1.5)).toThrow(expect.objectContaining({ reason: 'generic' }))
  })

  it('accepts whole credits from 1 up to the catalog ceiling', () => {
    expect(MAX_SALE_CREDITS).toBe(1_000_000_000_000n)
    expect(isValidCredits(1)).toBe(true)
    expect(isValidCredits(Number(MAX_SALE_CREDITS))).toBe(true)
    expect(isValidCredits(Number(MAX_SALE_CREDITS) + 1)).toBe(false)
    expect(isValidCredits(0)).toBe(false)
    expect(isValidCredits(1.5)).toBe(false)
  })

  it('takes MANA amounts with up to two decimals, from 1 MANA up to the catalog ceiling', () => {
    expect(sanitizeManaInput('1a.2.345')).toBe('1.23')
    expect(sanitizeManaInput('.5')).toBe('.5')
    expect(parseManaAmount('1.5')).toBe(1_500_000_000_000_000_000n)
    expect(parseManaAmount('.5')).toBe(500_000_000_000_000_000n)
    expect(parseManaAmount('')).toBeNull()
    expect(parseManaAmount('1.')).toBe(1_000_000_000_000_000_000n)
    expect(parseManaAmount('0.' + '9'.repeat(19))).toBeNull()
    expect(isValidManaWei(MIN_SALE_MANA_WEI)).toBe(true)
    expect(isValidManaWei(MIN_SALE_MANA_WEI - 1n)).toBe(false)
    expect(isValidManaWei(MAX_SALE_MANA_WEI)).toBe(true)
    expect(isValidManaWei(MAX_SALE_MANA_WEI + 1n)).toBe(false)
    expect(toPricedSale('mana', '2.5')).toEqual({ kind: 'mana', manaWei: 2_500_000_000_000_000_000n })
    expect(toPricedSale('mana', '0.99')).toBeNull()
    expect(toPricedSale('credits', '50')).toEqual({ kind: 'credits', credits: 50 })
    expect(toPricedSale('credits', '')).toBeNull()
    expect(toPricedSale('credits', '1.5')).toBeNull()
  })

  it('estimates the USD value of a MANA amount at a USD-wei-per-MANA rate', () => {
    expect(formatManaAsUsd(10n ** 18n, 300_000_000_000_000_000n)).toBe('$0.30')
    expect(formatManaAsUsd(2_500_000_000_000_000_000n, 300_000_000_000_000_000n)).toBe('$0.75')
  })

  it('compares a new price against the listing it replaces', () => {
    expect(listingToSalePrice({ itemId: '3', currency: 'credits', credits: 50 })).toEqual({
      kind: 'credits',
      credits: 50
    })
    expect(listingToSalePrice({ itemId: '3', currency: 'mana', manaWei: 0n })).toEqual({ kind: 'free' })
    expect(isSamePrice({ kind: 'credits', credits: 50 }, { kind: 'credits', credits: 50 })).toBe(true)
    expect(isSamePrice({ kind: 'credits', credits: 50 }, { kind: 'mana', manaWei: 50n * 10n ** 18n })).toBe(false)
    expect(isSamePrice({ kind: 'mana', manaWei: 5n }, { kind: 'mana', manaWei: 6n })).toBe(false)
    expect(isSamePrice({ kind: 'mana', manaWei: 5n }, { kind: 'mana', manaWei: 5n })).toBe(true)
    expect(isSamePrice({ kind: 'free' }, { kind: 'free' })).toBe(true)
  })

  it('validates wallet addresses', () => {
    expect(isValidAddress(BENEFICIARY)).toBe(true)
    expect(isValidAddress('0x123')).toBe(false)
    expect(isValidAddress('pirate.dcl.eth')).toBe(false)
    expect(isValidAddress(BENEFICIARY.toUpperCase().replace('0X', '0x'))).toBe(true)
    expect(isValidAddress('0x19E7E376E7C213B7E7e7e46cc70A5dD086DAff2a')).toBe(false)
    expect(isValidAddress(BENEFICIARY.slice(2))).toBe(false)
  })

  it('expires at the end of the picked day, and only accepts dates from tomorrow on', () => {
    const now = new Date(2026, 8, 10, 12, 0, 0).getTime()
    expect(formatDateValue(minExpirationDate(now))).toBe('2026-09-11')
    const picked = parseExpirationDate('2026-09-11')!
    expect(new Date(picked).getHours()).toBe(23)
    expect(parseExpirationDate('09/11/2026')).toBeNull()
    expect(parseExpirationDate('')).toBeNull()
  })
})

describe('enabling sales', () => {
  it('is enabled once the marketplace new orders are signed on is a minter, whatever the casing', () => {
    expect(isSalesEnabled(collection, CHAIN_ID)).toBe(false)
    expect(
      isSalesEnabled({ ...collection, minters: [MARKETPLACE_V3.toUpperCase().replace('0X', '0x')] }, CHAIN_ID)
    ).toBe(true)
  })

  it('is not enabled when only an older marketplace is a minter, so sales are enabled again for the new one', () => {
    expect(isSalesEnabled({ ...collection, minters: [MARKETPLACE_V2, MARKETPLACE_V1] }, CHAIN_ID)).toBe(false)
    expect(isSalesEnabled({ ...collection, minters: ['0x00000000000000000000000000000000000000dd'] }, CHAIN_ID)).toBe(
      false
    )
  })

  it('knows a collection already sells on an older marketplace, whatever the casing', () => {
    expect(isSellingOnOlderMarketplace({ ...collection, minters: [MARKETPLACE_V1] }, CHAIN_ID)).toBe(true)
    expect(
      isSellingOnOlderMarketplace(
        { ...collection, minters: [MARKETPLACE_V2.toUpperCase().replace('0X', '0x')] },
        CHAIN_ID
      )
    ).toBe(true)
  })

  it('does not count the current marketplace or any other minter as an older marketplace', () => {
    expect(isSellingOnOlderMarketplace(collection, CHAIN_ID)).toBe(false)
    expect(
      isSellingOnOlderMarketplace(
        { ...collection, minters: [MARKETPLACE_V3, '0x00000000000000000000000000000000000000dd'] },
        CHAIN_ID
      )
    ).toBe(false)
  })

  it('builds setMinters([marketplace], [true]) on the collection contract', () => {
    expect(toTransaction(buildEnableSalesCall(collection, CHAIN_ID))).toEqual({
      to: '0x00000000000000000000000000000000000000cc',
      data:
        '0x41bceced' +
        '0000000000000000000000000000000000000000000000000000000000000040' +
        '0000000000000000000000000000000000000000000000000000000000000080' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '00000000000000000000000036fd1434a6c4b8ade80c9847c1d15033ce34488c' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000001'
    })
  })

  it('refuses a collection without a contract', () => {
    expect(() => buildEnableSalesCall({ ...collection, contractAddress: undefined }, CHAIN_ID)).toThrow(SellItemError)
  })

  it('patches the minters the way the server will report them', () => {
    const enabled = withSalesEnabled(collection, CHAIN_ID)
    expect(isSalesEnabled(enabled, CHAIN_ID)).toBe(true)
    expect(withSalesEnabled(enabled, CHAIN_ID)).toBe(enabled)
  })
})

describe('buildItemOrder', () => {
  it('sells every remaining unit for USD-pegged MANA, paid to the beneficiary', () => {
    const order = buildItemOrder(params, indexes, SALT, 1000)
    expect(order).toEqual({
      signer: ADDRESS,
      network: Network.MATIC,
      chainId: CHAIN_ID,
      type: TradeType.PUBLIC_ITEM_ORDER,
      checks: {
        uses: 90,
        expiration: NO_EXPIRATION,
        effective: 1000,
        salt: SALT,
        contractSignatureIndex: 1,
        signerSignatureIndex: 2,
        allowedRoot: '0x',
        externalChecks: []
      },
      sent: [{ assetType: TradeAssetType.COLLECTION_ITEM, contractAddress: CONTRACT, itemId: '3', extra: '' }],
      received: [
        {
          assetType: TradeAssetType.USD_PEGGED_MANA,
          contractAddress: MANA,
          amount: '5000000000000000000',
          extra: '',
          beneficiary: BENEFICIARY
        }
      ]
    })
  })

  it('sells for plain MANA as an ERC20 order, like the legacy builder', () => {
    const order = buildItemOrder(
      { ...params, price: { kind: 'mana', manaWei: 2_500_000_000_000_000_000n } },
      indexes,
      SALT
    )
    expect(order.received).toEqual([
      {
        assetType: TradeAssetType.ERC20,
        contractAddress: MANA,
        amount: '2500000000000000000',
        extra: '',
        beneficiary: BENEFICIARY
      }
    ])
  })

  it('encodes a giveaway as zero MANA to nobody, the way the legacy builder does', () => {
    const order = buildItemOrder({ ...params, price: { kind: 'free' } }, indexes, SALT)
    expect(order.received).toEqual([
      {
        assetType: TradeAssetType.ERC20,
        contractAddress: MANA,
        amount: '0',
        extra: '',
        beneficiary: zeroAddress
      }
    ])
  })

  it('refuses unpublished and sold-out items', () => {
    expect(() => buildItemOrder({ ...params, item: { ...item, tokenId: undefined } }, indexes, SALT)).toThrow(
      expect.objectContaining({ reason: 'not_published' })
    )
    expect(() => buildItemOrder({ ...params, item: { ...item, totalSupply: 100 } }, indexes, SALT)).toThrow(
      expect.objectContaining({ reason: 'sold_out' })
    )
  })
})

describe('sellItem', () => {
  function makeDeps(overrides: Partial<SellItemDeps> = {}): SellItemDeps {
    return {
      fetchSignatureIndexes: vi.fn().mockResolvedValue(indexes),
      signTrade: vi.fn().mockResolvedValue('0xsignature'),
      createTrade: vi.fn().mockResolvedValue('trade-1'),
      onSigned: vi.fn(),
      ...overrides
    }
  }

  it('reads the indexes, signs the order, reports the signature and stores the order', async () => {
    const deps = makeDeps()
    const result = await sellItem(params, deps)

    expect(deps.fetchSignatureIndexes).toHaveBeenCalledWith(ADDRESS, CHAIN_ID)
    const signed = vi.mocked(deps.signTrade).mock.calls[0][0]
    expect(signed.checks).toMatchObject(indexes)
    expect(signed.checks.salt).toMatch(/^0x[0-9a-f]{64}$/)
    expect(deps.onSigned).toHaveBeenCalledTimes(1)
    expect(deps.createTrade).toHaveBeenCalledWith({ ...signed, signature: '0xsignature' })
    expect(result).toEqual({
      itemId: '3',
      tradeId: 'trade-1',
      tradeContract: MARKETPLACE_V3.toLowerCase(),
      currency: 'credits',
      credits: 50
    })
  })

  it('stores the order exactly as signed, its amount as a decimal string', async () => {
    vi.useFakeTimers({ now: Date.UTC(2026, 9, 16, 12), toFake: ['Date'] })
    const createTrade = vi.fn().mockResolvedValue('trade-1')
    const deps = makeDeps({
      randomSalt: () => SALT,
      signTrade: tradeSigner(signTypedDataWithTestKey, CHAIN_ID),
      createTrade
    })
    try {
      await sellItem({ ...params, address: TEST_SIGNER.toLowerCase() }, deps)
    } finally {
      vi.useRealTimers()
    }
    expect(createTrade.mock.calls[0][0]).toEqual({
      signer: '0x19e7e376e7c213b7e7e7e46cc70a5dd086daff2a',
      network: 'MATIC',
      chainId: 80002,
      type: 'public_item_order',
      checks: {
        uses: 90,
        expiration: 4102444800000,
        effective: 1792152000000,
        salt: '0x1111111111111111111111111111111111111111111111111111111111111111',
        contractSignatureIndex: 1,
        signerSignatureIndex: 2,
        allowedRoot: '0x',
        externalChecks: []
      },
      sent: [{ assetType: 4, contractAddress: '0x00000000000000000000000000000000000000cc', itemId: '3', extra: '' }],
      received: [
        {
          assetType: 2,
          contractAddress: '0x7ad72b9f944ea9793cf4055d88f81138cc2c63a0',
          amount: '5000000000000000000',
          extra: '',
          beneficiary: '0x00000000000000000000000000000000000000bb'
        }
      ],
      signature:
        '0xbd3d350e94e601edb40e952b98a2091c71cd595059c99e095467b5dca0b3de66320e714fe18b7bff5e8726d93040b5244f8db43cc09fe62440146eb05b775b3c1c'
    })
  })

  it('answers a free listing for a giveaway and a MANA listing for a MANA price', async () => {
    const free = await sellItem({ ...params, price: { kind: 'free' } }, makeDeps())
    expect(free).toEqual({
      itemId: '3',
      tradeId: 'trade-1',
      tradeContract: MARKETPLACE_V3.toLowerCase(),
      currency: 'mana',
      manaWei: 0n
    })
    const mana = await sellItem({ ...params, price: { kind: 'mana', manaWei: 10n ** 18n } }, makeDeps())
    expect(mana).toEqual({
      itemId: '3',
      tradeId: 'trade-1',
      tradeContract: MARKETPLACE_V3.toLowerCase(),
      currency: 'mana',
      manaWei: 10n ** 18n
    })
  })

  it('maps a dismissed wallet prompt to a rejection without storing anything', async () => {
    const deps = makeDeps({ signTrade: vi.fn().mockRejectedValue({ code: 4001, message: 'User rejected' }) })
    await expect(sellItem(params, deps)).rejects.toMatchObject({ reason: 'rejected' })
    expect(deps.onSigned).not.toHaveBeenCalled()
    expect(deps.createTrade).not.toHaveBeenCalled()
  })

  it('keeps the sequence errors and wraps anything else as generic', () => {
    expect(toSellItemError(new SellItemError('sold_out')).reason).toBe('sold_out')
    expect(toSellItemError(new Error('500')).reason).toBe('generic')
    expect(toSellItemError('ACTION_REJECTED')).toMatchObject({ reason: 'generic' })
  })

  it('tells a wallet on another account or network apart, so the creator is asked to switch it back', () => {
    expect(toSellItemError(new WrongAccountError(ADDRESS, BENEFICIARY)).reason).toBe('wallet')
    expect(toSellItemError(new WrongChainError(CHAIN_ID, 1)).reason).toBe('wallet')
  })
})

const storedTrade: Trade = {
  ...buildItemOrder(params, indexes, SALT, 1000),
  id: 'trade-1',
  signature: '0x' + 'ab'.repeat(65),
  createdAt: 1,
  contract: MARKETPLACE_V2
}

const REF = { tradeId: 'trade-1', contractAddress: CONTRACT, itemId: '3' }

describe('removing a listing', () => {
  it('cancels the stored order on the marketplace it was signed for', () => {
    expect(toTransaction(buildCancelListingCall(storedTrade))).toEqual({
      to: '0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7',
      data:
        '0x6f773280' +
        '0000000000000000000000000000000000000000000000000000000000000020' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000020' +
        '00000000000000000000000000000000000000000000000000000000000000aa' +
        '00000000000000000000000000000000000000000000000000000000000000a0' +
        '0000000000000000000000000000000000000000000000000000000000000120' +
        '0000000000000000000000000000000000000000000000000000000000000280' +
        '0000000000000000000000000000000000000000000000000000000000000380' +
        '0000000000000000000000000000000000000000000000000000000000000041' +
        'abababababababababababababababababababababababababababababababab' +
        'abababababababababababababababababababababababababababababababab' +
        'ab00000000000000000000000000000000000000000000000000000000000000' +
        '000000000000000000000000000000000000000000000000000000000000005a' +
        '00000000000000000000000000000000000000000000000000000000f4865700' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '1111111111111111111111111111111111111111111111111111111111111111' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000002' +
        '0000000000000000000000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000120' +
        '0000000000000000000000000000000000000000000000000000000000000140' +
        '0000000000000000000000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000020' +
        '0000000000000000000000000000000000000000000000000000000000000004' +
        '00000000000000000000000000000000000000000000000000000000000000cc' +
        '0000000000000000000000000000000000000000000000000000000000000003' +
        '0000000000000000000000000000000000000000000000000000000000000000' +
        '00000000000000000000000000000000000000000000000000000000000000a0' +
        '0000000000000000000000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000020' +
        '0000000000000000000000000000000000000000000000000000000000000002' +
        '0000000000000000000000007ad72b9f944ea9793cf4055d88f81138cc2c63a0' +
        '0000000000000000000000000000000000000000000000004563918244f40000' +
        '00000000000000000000000000000000000000000000000000000000000000bb' +
        '00000000000000000000000000000000000000000000000000000000000000a0' +
        '0000000000000000000000000000000000000000000000000000000000000000'
    })
  })

  it('reads the order, sends the cancellation, reports the signature and waits for the receipt', async () => {
    const deps = {
      fetchTrade: vi.fn().mockResolvedValue(storedTrade),
      fetchItemTradeId: vi.fn().mockResolvedValue(null),
      sendTransaction: vi.fn().mockResolvedValue('0xhash'),
      waitForTransaction: vi.fn().mockResolvedValue(true),
      onSigned: vi.fn()
    }
    await removeListing(REF, deps)
    expect(deps.fetchTrade).toHaveBeenCalledWith('trade-1')
    expect(deps.sendTransaction.mock.calls[0][0]).toMatchObject({ method: 'cancelSignature' })
    expect(deps.onSigned).toHaveBeenCalledTimes(1)
    expect(deps.waitForTransaction).toHaveBeenCalledWith('0xhash')

    deps.waitForTransaction.mockResolvedValue(false)
    await expect(removeListing(REF, deps)).rejects.toMatchObject({ reason: 'generic' })
    deps.sendTransaction.mockRejectedValue({ code: 4001 })
    await expect(removeListing(REF, deps)).rejects.toMatchObject({ reason: 'rejected' })
  })
})

describe('a stale listing id', () => {
  const base = {
    sendTransaction: vi.fn().mockResolvedValue('0xhash'),
    waitForTransaction: vi.fn().mockResolvedValue(true)
  }

  it("falls back to the item's current order when the server retired the cached one", async () => {
    const fetchTrade = vi.fn(async (id: string) => {
      if (id === 'trade-1') throw new TradeNotFoundError(id)
      return { ...storedTrade, id }
    })
    const fetchItemTradeId = vi.fn().mockResolvedValue('trade-9')
    await removeListing(REF, { ...base, fetchTrade, fetchItemTradeId })
    expect(fetchItemTradeId).toHaveBeenCalledWith(CONTRACT, '3')
    expect(fetchTrade).toHaveBeenLastCalledWith('trade-9')
    expect(base.sendTransaction).toHaveBeenCalled()
  })

  it('reports an item that is no longer on sale instead of cancelling anything', async () => {
    const fetchTrade = vi.fn().mockRejectedValue(new TradeNotFoundError('trade-1'))
    await expect(
      removeListing(REF, {
        ...base,
        sendTransaction: vi.fn(),
        fetchTrade,
        fetchItemTradeId: vi.fn().mockResolvedValue(null)
      })
    ).rejects.toMatchObject({ reason: 'not_listed' })
  })
})

describe('updating the price', () => {
  it('keeps the beneficiary and expiration of the old order, paying the signer for a giveaway', () => {
    expect(getListingTerms(storedTrade, ADDRESS)).toEqual({ beneficiary: BENEFICIARY, expiresAt: NO_EXPIRATION })
    const free = buildItemOrder({ ...params, price: { kind: 'free' } }, indexes, SALT)
    expect(getListingTerms({ ...storedTrade, ...free }, ADDRESS).beneficiary).toBe(ADDRESS)
  })

  it('cancels the old order, then signs and stores a new one at the new price with the same terms', async () => {
    const deps = {
      fetchTrade: vi.fn().mockResolvedValue(storedTrade),
      fetchItemTradeId: vi.fn().mockResolvedValue(null),
      sendTransaction: vi.fn().mockResolvedValue('0xhash'),
      waitForTransaction: vi.fn().mockResolvedValue(true),
      fetchSignatureIndexes: vi.fn().mockResolvedValue(indexes),
      signTrade: vi.fn().mockResolvedValue('0xnewsig'),
      createTrade: vi.fn().mockResolvedValue('trade-2'),
      onSigned: vi.fn(),
      onCancelled: vi.fn()
    }
    const listing = await updatePrice(
      {
        address: ADDRESS,
        chainId: CHAIN_ID,
        collection,
        item,
        tradeId: 'trade-1',
        price: { kind: 'credits', credits: 80 }
      },
      deps
    )

    expect(deps.onSigned.mock.calls.map(c => c[0])).toEqual(['cancel', 'sign'])
    expect(deps.onCancelled).toHaveBeenCalledWith({ beneficiary: BENEFICIARY, expiresAt: NO_EXPIRATION })
    const signed = deps.signTrade.mock.calls[0][0]
    expect(signed.received[0]).toMatchObject({ amount: creditsToUsdWei(80), beneficiary: BENEFICIARY })
    expect(signed.checks.expiration).toBe(NO_EXPIRATION)
    expect(listing).toEqual({
      itemId: '3',
      tradeId: 'trade-2',
      tradeContract: MARKETPLACE_V3.toLowerCase(),
      currency: 'credits',
      credits: 80
    })
  })

  it('can switch the listing to MANA, keeping the same terms', async () => {
    const deps = {
      fetchTrade: vi.fn().mockResolvedValue(storedTrade),
      fetchItemTradeId: vi.fn().mockResolvedValue(null),
      sendTransaction: vi.fn().mockResolvedValue('0xhash'),
      waitForTransaction: vi.fn().mockResolvedValue(true),
      fetchSignatureIndexes: vi.fn().mockResolvedValue(indexes),
      signTrade: vi.fn().mockResolvedValue('0xnewsig'),
      createTrade: vi.fn().mockResolvedValue('trade-2')
    }
    const listing = await updatePrice(
      {
        address: ADDRESS,
        chainId: CHAIN_ID,
        collection,
        item,
        tradeId: 'trade-1',
        price: { kind: 'mana', manaWei: 10n ** 18n }
      },
      deps
    )
    expect(deps.signTrade.mock.calls[0][0].received[0]).toMatchObject({
      assetType: TradeAssetType.ERC20,
      amount: '1000000000000000000',
      beneficiary: BENEFICIARY
    })
    expect(listing).toEqual({
      itemId: '3',
      tradeId: 'trade-2',
      tradeContract: MARKETPLACE_V3.toLowerCase(),
      currency: 'mana',
      manaWei: 10n ** 18n
    })
  })

  it('refuses a sold-out item before cancelling its order', async () => {
    const deps = {
      fetchTrade: vi.fn(),
      fetchItemTradeId: vi.fn(),
      sendTransaction: vi.fn(),
      waitForTransaction: vi.fn(),
      fetchSignatureIndexes: vi.fn(),
      signTrade: vi.fn(),
      createTrade: vi.fn()
    }
    await expect(
      updatePrice(
        {
          address: ADDRESS,
          chainId: CHAIN_ID,
          collection,
          item: { ...item, totalSupply: 100 },
          tradeId: 'trade-1',
          price: { kind: 'credits', credits: 80 }
        },
        deps
      )
    ).rejects.toMatchObject({ reason: 'sold_out' })
    expect(deps.sendTransaction).not.toHaveBeenCalled()
  })

  it('stops before cancelling anything when the wallet prompt is dismissed', async () => {
    const deps = {
      fetchTrade: vi.fn().mockResolvedValue(storedTrade),
      fetchItemTradeId: vi.fn(),
      sendTransaction: vi.fn().mockRejectedValue({ code: 4001 }),
      waitForTransaction: vi.fn(),
      fetchSignatureIndexes: vi.fn(),
      signTrade: vi.fn(),
      createTrade: vi.fn(),
      onCancelled: vi.fn()
    }
    await expect(
      updatePrice(
        {
          address: ADDRESS,
          chainId: CHAIN_ID,
          collection,
          item,
          tradeId: 'trade-1',
          price: { kind: 'credits', credits: 80 }
        },
        deps
      )
    ).rejects.toMatchObject({ reason: 'rejected' })
    expect(deps.onCancelled).not.toHaveBeenCalled()
    expect(deps.signTrade).not.toHaveBeenCalled()
  })
})

describe('withConflictRetry', () => {
  const conflict = new TradeConflictError('There is already an open order for this Item')

  it('re-posts the same signed order while the server still sees the old one', async () => {
    const createTrade = vi
      .fn()
      .mockRejectedValueOnce(conflict)
      .mockRejectedValueOnce(conflict)
      .mockResolvedValue('trade-2')
    const wait = vi.fn().mockResolvedValue(undefined)
    const post = withConflictRetry(createTrade, wait, [10, 20, 30])
    const signed = { signature: '0x1' } as never
    await expect(post(signed)).resolves.toBe('trade-2')
    expect(createTrade).toHaveBeenCalledTimes(3)
    expect(createTrade.mock.calls.every(call => call[0] === signed)).toBe(true)
    expect(wait.mock.calls.map(c => c[0])).toEqual([10, 20])
  })

  it('gives up after the last delay and never retries other failures', async () => {
    const wait = vi.fn().mockResolvedValue(undefined)
    const stuck = withConflictRetry(vi.fn().mockRejectedValue(conflict), wait, [1, 1])
    await expect(stuck({} as never)).rejects.toBe(conflict)
    expect(wait).toHaveBeenCalledTimes(2)
    const other = vi.fn().mockRejectedValue(new Error('Invalid signature'))
    await expect(withConflictRetry(other, wait, [1])({} as never)).rejects.toThrow('Invalid signature')
    expect(other).toHaveBeenCalledTimes(1)
  })
})

describe('removing a legacy store listing', () => {
  const metadata = '1:w:Pirate Hat:Yarr:hat:BaseMale'
  // The collection's `items` outputs: rarity, maxSupply, totalSupply, price, beneficiary, metadata, contentHash.
  const onChain = ['epic', 1000n, 3n, 10n ** 18n, BENEFICIARY, metadata, '']

  it('reads the item on chain and clears only its price, keeping beneficiary and metadata', async () => {
    const deps = {
      readContract: vi.fn().mockResolvedValue(onChain),
      sendTransaction: vi.fn().mockResolvedValue('0xhash'),
      waitForTransaction: vi.fn().mockResolvedValue(true),
      onSigned: vi.fn()
    }
    await removeStoreListing(collection, item, CHAIN_ID, deps)
    expect(deps.readContract).toHaveBeenCalledWith(expect.objectContaining({ address: CONTRACT }), 'items', ['3'])
    expect(toTransaction(deps.sendTransaction.mock.calls[0][0])).toEqual({
      to: '0x00000000000000000000000000000000000000cc',
      data:
        '0x0ebde06e' +
        '0000000000000000000000000000000000000000000000000000000000000080' +
        '00000000000000000000000000000000000000000000000000000000000000c0' +
        '0000000000000000000000000000000000000000000000000000000000000100' +
        '0000000000000000000000000000000000000000000000000000000000000140' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000003' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '00000000000000000000000000000000000000000000000000000000000000bb' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000020' +
        '0000000000000000000000000000000000000000000000000000000000000020' +
        '313a773a506972617465204861743a596172723a6861743a426173654d616c65'
    })
    expect(deps.onSigned).toHaveBeenCalledTimes(1)
    expect(deps.waitForTransaction).toHaveBeenCalledWith('0xhash')
  })

  it('maps a dismissed prompt and a revert like the other sale actions', async () => {
    const deps = {
      readContract: vi.fn().mockResolvedValue(onChain),
      sendTransaction: vi.fn().mockRejectedValue({ code: 4001 }),
      waitForTransaction: vi.fn().mockResolvedValue(false)
    }
    await expect(removeStoreListing(collection, item, CHAIN_ID, deps)).rejects.toMatchObject({ reason: 'rejected' })
    deps.sendTransaction.mockResolvedValue('0xhash')
    await expect(removeStoreListing(collection, item, CHAIN_ID, deps)).rejects.toMatchObject({ reason: 'generic' })
  })
})
