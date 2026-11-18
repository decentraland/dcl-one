import { afterEach, describe, expect, it, vi } from 'vitest'
import { Network, TradeAssetType, TradeType, type Trade, type TradeCreation } from '@dcl/schemas'
import { ContractName, getContract } from '~/lib/chain/contracts'
import { zeroAddress } from 'viem'

vi.mock('~/lib/auth', async () => {
  const actual = await vi.importActual<typeof import('~/lib/auth')>('~/lib/auth')
  return { ...actual, readContract: vi.fn(), signedFetch: vi.fn() }
})

import { readContract, signedFetch } from '~/lib/auth'
import { typedDataDigest } from '~/test/chain'
import {
  OFFCHAIN_MARKETPLACE_TYPES,
  TradeConflictError,
  TradeNotFoundError,
  UnknownTradeContractError,
  createTrade,
  fetchSignatureIndexes,
  fetchTrade,
  getTradeContract,
  getTradeDomain,
  toOnChainTrade,
  toTradeTypedValues,
  type UnsignedTrade
} from './trades'

const CHAIN_ID = 80002
const SIGNER = '0x00000000000000000000000000000000000000aa'
const COLLECTION = '0x00000000000000000000000000000000000000cc'
const MANA = getContract(ContractName.MANAToken, CHAIN_ID).address

const trade: UnsignedTrade = {
  signer: SIGNER,
  network: Network.MATIC,
  chainId: CHAIN_ID,
  type: TradeType.PUBLIC_ITEM_ORDER,
  checks: {
    uses: 90,
    expiration: 4102444800000,
    effective: 1757500000123,
    salt: '0x01',
    contractSignatureIndex: 2,
    signerSignatureIndex: 3,
    allowedRoot: '0x',
    externalChecks: []
  },
  sent: [{ assetType: TradeAssetType.COLLECTION_ITEM, contractAddress: COLLECTION, itemId: '7', extra: '' }],
  received: [
    {
      assetType: TradeAssetType.USD_PEGGED_MANA,
      contractAddress: MANA,
      amount: '500000000000000000',
      extra: '',
      beneficiary: SIGNER
    }
  ]
}

afterEach(() => vi.mocked(signedFetch).mockReset())

describe('getTradeDomain', () => {
  it('names the newest marketplace contract and carries the chain id in the salt', () => {
    const contract = getContract(ContractName.OffChainMarketplaceV3, CHAIN_ID)
    expect(getTradeDomain(CHAIN_ID)).toEqual({
      name: contract.name,
      version: contract.version,
      salt: '0x' + CHAIN_ID.toString(16).padStart(64, '0'),
      verifyingContract: contract.address
    })
  })
})

describe('toTradeTypedValues', () => {
  it('produces a struct the marketplace types can hash: seconds, padded bytes32, flattened assets', () => {
    const values = toTradeTypedValues(trade)
    expect(values).toEqual({
      checks: {
        uses: 90,
        expiration: 4102444800,
        effective: 1757500000,
        salt: '0x' + '00'.repeat(31) + '01',
        contractSignatureIndex: 2,
        signerSignatureIndex: 3,
        allowedRoot: '0x' + '00'.repeat(32),
        externalChecks: []
      },
      sent: [{ assetType: 4, contractAddress: COLLECTION, value: '7', extra: '0x' }],
      received: [{ assetType: 2, contractAddress: MANA, value: '500000000000000000', extra: '0x', beneficiary: SIGNER }]
    })
    // What the wallet signs and the marketplace recovers the signer from.
    expect(typedDataDigest(getTradeDomain(CHAIN_ID), OFFCHAIN_MARKETPLACE_TYPES, values)).toBe(
      '0xfc5244b13838fde268d232e2e06c9720b233590078a5dbd7c2ce17bebe156e9f'
    )
  })
})

describe('fetchSignatureIndexes', () => {
  it('reads both indexes from the marketplace contract', async () => {
    vi.mocked(readContract).mockImplementation(async (_contract, method) =>
      method === 'contractSignatureIndex' ? 1n : 4n
    )
    await expect(fetchSignatureIndexes(SIGNER, CHAIN_ID)).resolves.toEqual({
      contractSignatureIndex: 1,
      signerSignatureIndex: 4
    })
    const contract = getContract(ContractName.OffChainMarketplaceV3, CHAIN_ID)
    expect(readContract).toHaveBeenCalledWith(contract, 'contractSignatureIndex')
    expect(readContract).toHaveBeenCalledWith(contract, 'signerSignatureIndex', [SIGNER])
  })
})

describe('createTrade', () => {
  const signed: TradeCreation = { ...trade, signature: '0xsig' }

  it('posts the signed order to marketplace-server as the builder and answers the trade id', async () => {
    vi.mocked(signedFetch).mockResolvedValue(
      new Response(JSON.stringify({ ok: true, data: { id: 'trade-1' } }), { status: 201 })
    )
    await expect(createTrade(SIGNER, signed)).resolves.toBe('trade-1')
    expect(signedFetch).toHaveBeenCalledWith(
      SIGNER,
      'https://marketplace-api.decentraland.zone',
      '/v1/trades',
      expect.objectContaining({ method: 'POST' }),
      { signer: 'dcl:builder', intent: 'dcl:create-trade' }
    )
    // Amounts stay decimal strings on the wire.
    expect(vi.mocked(signedFetch).mock.calls[0][3]?.body).toBe(
      JSON.stringify({
        signer: '0x00000000000000000000000000000000000000aa',
        network: 'MATIC',
        chainId: 80002,
        type: 'public_item_order',
        checks: {
          uses: 90,
          expiration: 4102444800000,
          effective: 1757500000123,
          salt: '0x01',
          contractSignatureIndex: 2,
          signerSignatureIndex: 3,
          allowedRoot: '0x',
          externalChecks: []
        },
        sent: [{ assetType: 4, contractAddress: '0x00000000000000000000000000000000000000cc', itemId: '7', extra: '' }],
        received: [
          {
            assetType: 2,
            contractAddress: '0x7ad72b9f944ea9793cf4055d88f81138cc2c63a0',
            amount: '500000000000000000',
            extra: '',
            beneficiary: '0x00000000000000000000000000000000000000aa'
          }
        ],
        signature: '0xsig'
      })
    )
  })

  it('surfaces the server message when the order is rejected', async () => {
    vi.mocked(signedFetch).mockResolvedValue(
      new Response(JSON.stringify({ ok: false, message: 'Invalid signature' }), { status: 400 })
    )
    await expect(createTrade(SIGNER, signed)).rejects.toThrow('Invalid signature')

    vi.mocked(signedFetch).mockResolvedValue(
      new Response(JSON.stringify({ ok: false, message: 'There is already an open order for this Item' }), {
        status: 409
      })
    )
    await expect(createTrade(SIGNER, signed)).rejects.toBeInstanceOf(TradeConflictError)
  })
})

const V2 = getContract(ContractName.OffChainMarketplaceV2, CHAIN_ID)
const stored: Trade = { ...trade, id: 'trade-1', signature: '0xsig', createdAt: 1, contract: V2.address }

describe('toOnChainTrade', () => {
  it('carries the signature, an empty allowed proof and nobody as the sent beneficiary', () => {
    const onChain = toOnChainTrade(stored)
    expect(onChain.signer).toBe(SIGNER)
    expect(onChain.signature).toBe('0xsig')
    expect(onChain.checks.allowedProof).toEqual([])
    expect(onChain.checks.expiration).toBe(4102444800)
    expect(onChain.sent[0]).toMatchObject({ value: '7', beneficiary: zeroAddress })
    expect(onChain.received[0]).toMatchObject({ beneficiary: SIGNER })
  })
})

describe('getTradeContract', () => {
  it('resolves the canonical marketplace of each generation from the stored order', () => {
    const v1 = getContract(ContractName.OffChainMarketplace, CHAIN_ID)
    expect(getTradeContract({ contract: v1.address, chainId: CHAIN_ID })).toMatchObject({
      name: v1.name,
      address: v1.address
    })
    expect(
      getTradeContract({ contract: V2.address.toUpperCase().replace('0X', '0x'), chainId: CHAIN_ID })
    ).toMatchObject({ name: V2.name, address: V2.address })
    const v3 = getContract(ContractName.OffChainMarketplaceV3, CHAIN_ID)
    expect(getTradeContract({ contract: v3.address, chainId: CHAIN_ID })).toMatchObject({ address: v3.address })
  })

  it('refuses to target an address the client does not know, another Decentraland contract or another chain', () => {
    expect(() =>
      getTradeContract({ contract: '0x00000000000000000000000000000000000000ee', chainId: CHAIN_ID })
    ).toThrow(UnknownTradeContractError)
    expect(() => getTradeContract({ contract: MANA, chainId: CHAIN_ID })).toThrow(UnknownTradeContractError)
    const mainnetV2 = getContract(ContractName.OffChainMarketplaceV2, 137)
    expect(() => getTradeContract({ contract: mainnetV2.address, chainId: 137 })).toThrow(UnknownTradeContractError)
  })
})

describe('fetchTrade', () => {
  it('reads the stored order and fails loudly when it is missing', async () => {
    const fetchMock = vi.fn()
    vi.stubGlobal('fetch', fetchMock)
    fetchMock.mockResolvedValueOnce(new Response(JSON.stringify({ ok: true, data: stored }), { status: 200 }))
    await expect(fetchTrade('trade-1')).resolves.toMatchObject({ id: 'trade-1' })
    expect(fetchMock.mock.calls[0][0]).toBe('https://marketplace-api.decentraland.zone/v1/trades/trade-1')
    fetchMock.mockResolvedValueOnce(new Response(JSON.stringify({ ok: false }), { status: 404 }))
    await expect(fetchTrade('nope')).rejects.toBeInstanceOf(TradeNotFoundError)
    vi.unstubAllGlobals()
  })
})
