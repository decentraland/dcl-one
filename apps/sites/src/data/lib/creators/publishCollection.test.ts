import { describe, expect, it, vi } from 'vitest'
import { ContractName, getContract } from '~/lib/chain/contracts'
import { decodeAbiParameters, maxUint256, parseAbiParameters, toFunctionSelector } from 'viem'
import { decodeCall, toTransaction } from '~/test/chain'
import { BuilderServerError } from './builder'
import { type Collection } from './collections'
import { CreditsServerError } from './credits'
import { ItemType, type Item, BODY_SHAPE_FEMALE } from './items'
import { getPublicationFee } from './publishFee'
import { WrongAccountError, WrongChainError } from './walletErrors'
import {
  MAX_PUBLISH_ITEMS,
  PublishCollectionError,
  buildCollectionManagerExternalCall,
  buildCreateCollectionArgs,
  buildUseCreditsCall,
  canPayWith,
  consolidatePublishedCollection,
  PublishTransactionRevertedError,
  getAvailablePaymentMethods,
  getManaApprovalNeeded,
  syncPublishedItems,
  getPublishBlocker,
  isRecentPublication,
  openFirstReviewRequest,
  publishCollection,
  toPublishError,
  type PublishDeps
} from './publishCollection'

const CHAIN_ID = 80002
const ADDRESS = '0x00000000000000000000000000000000000000aa'
const SALT = '0x' + '11'.repeat(32)
const EXTERNAL_SALT = '0x' + '22'.repeat(32)
const ETHER = 10n ** 18n

const collection: Collection = {
  id: 'col-1',
  name: 'Halloween',
  owner: ADDRESS,
  urn: 'urn:decentraland:amoy:collections-v2:0x0000000000000000000000000000000000000000',
  salt: SALT,
  isPublished: false,
  isApproved: false,
  itemCount: 2,
  minters: [],
  managers: [],
  createdAt: 1,
  updatedAt: 1
}

function makeItem(id: string, createdAt: number): Item {
  return {
    id,
    name: `Item ${id}`,
    description: '',
    thumbnail: 'thumbnail.png',
    owner: ADDRESS,
    collectionId: collection.id,
    rarity: 'legendary',
    price: maxUint256.toString(),
    beneficiary: ADDRESS,
    isPublished: false,
    isApproved: false,
    inCatalyst: false,
    type: ItemType.WEARABLE,
    data: {
      category: 'hat',
      representations: [{ bodyShapes: [BODY_SHAPE_FEMALE], mainFile: 'm.glb', contents: ['m.glb'] }]
    },
    contents: { 'm.glb': 'hash', 'thumbnail.png': 'thumb' },
    createdAt,
    updatedAt: createdAt
  }
}

const items = [makeItem('b', 2), makeItem('a', 1)]

const fee = getPublicationFee(
  [
    {
      id: 'epic',
      name: 'epic',
      price: '1',
      maxSupply: '1000',
      prices: { USD: (5n * ETHER).toString(), MANA: (500n * ETHER).toString() }
    }
  ],
  items.length
)!

const authorization = {
  credit: {
    id: '0x01',
    amount: (1000n * ETHER).toString(),
    expiresAt: '1700000000',
    signature: '0x' + 'ab'.repeat(65)
  },
  externalCallSignature: '0x' + 'cd'.repeat(65)
}

function makeDeps(overrides: Partial<PublishDeps> = {}): PublishDeps & { calls: string[] } {
  const calls: string[] = []
  const track =
    <T extends unknown[], R>(name: string, impl: (...args: T) => R) =>
    (...args: T) => {
      calls.push(name)
      return impl(...args)
    }
  return {
    calls,
    saveCollection: track('saveCollection', async (c: Collection) => ({
      ...c,
      salt: SALT,
      contractAddress: '0xc0ffee'
    })),
    fetchItems: track('fetchItems', async () => items),
    rehashItem: track('rehashItem', async (item: Item) => item),
    saveTOS: track('saveTOS', async () => undefined),
    authorizePublication: track('authorizePublication', async () => authorization),
    sendTransaction: track('sendTransaction', async () => '0xtx'),
    lockCollection: track('lockCollection', async () => 1234),
    ...overrides
  }
}

describe('getPublishBlocker', () => {
  it('only lets a draft with 1..50 items publish', () => {
    expect(getPublishBlocker(collection, 3, [])).toBeNull()
    expect(getPublishBlocker(collection, 0, [])).toBe('no_items')
    expect(getPublishBlocker(collection, MAX_PUBLISH_ITEMS + 1, [])).toBe('too_many_items')
    expect(getPublishBlocker({ ...collection, isPublished: true }, 3, [])).toBe('not_draft')
  })

  it('lets a locked draft publish again, since its last transaction may have reverted', () => {
    expect(getPublishBlocker({ ...collection, lock: Date.now() }, 3, [])).toBeNull()
    expect(getPublishBlocker({ ...collection, lock: Date.now(), isPublished: true }, 3, [])).toBe('not_draft')
  })

  it('blocks publishing while a smart wearable has no preview video', () => {
    const smart = makeItem('sw', 3)
    smart.contents = { ...smart.contents, 'male/bin/game.js': 'js' }
    expect(getPublishBlocker(collection, 1, [smart])).toBe('missing_smart_wearable_video')
    smart.contents['video.mp4'] = 'video'
    expect(getPublishBlocker(collection, 1, [smart])).toBeNull()
  })

  it('blocks publishing on invalid items only when nothing else blocks it first', () => {
    const invalid = { hasInvalidItems: true }
    expect(getPublishBlocker(collection, 3, [], invalid)).toBe('has_invalid_items')
    expect(getPublishBlocker(collection, 3, [], { hasInvalidItems: false })).toBeNull()
    expect(getPublishBlocker(collection, 0, [], invalid)).toBe('no_items')
    expect(getPublishBlocker(collection, MAX_PUBLISH_ITEMS + 1, [], invalid)).toBe('too_many_items')
    expect(getPublishBlocker({ ...collection, isPublished: true }, 3, [], invalid)).toBe('not_draft')
  })
})

describe('payment methods', () => {
  it('offers credits always and MANA only with a positive balance', () => {
    expect(getAvailablePaymentMethods(undefined)).toEqual(['credits'])
    expect(getAvailablePaymentMethods(0n)).toEqual(['credits'])
    expect(getAvailablePaymentMethods(1n)).toEqual(['credits', 'mana'])
  })

  it('leaves MANA as the only method, at any balance, when credits are turned off', () => {
    expect(getAvailablePaymentMethods(undefined, false)).toEqual(['mana'])
    expect(getAvailablePaymentMethods(0n, false)).toEqual(['mana'])
    expect(getAvailablePaymentMethods(1n, false)).toEqual(['mana'])
  })

  it('checks each method against its own balance', () => {
    expect(canPayWith('credits', fee, { credits: 100, manaWei: 0n })).toBe(true)
    expect(canPayWith('credits', fee, { credits: 99, manaWei: 0n })).toBe(false)
    expect(canPayWith('mana', fee, { credits: 0, manaWei: 1000n * ETHER })).toBe(true)
    expect(canPayWith('mana', fee, { credits: 0, manaWei: 999n * ETHER })).toBe(false)
  })
})

describe('getManaApprovalNeeded', () => {
  // What the contract pulls: 3% above a 1000 MANA quote, still within the verification tolerance.
  const onChainTotal = 1030n * ETHER
  const padded = 1081n * ETHER + ETHER / 2n

  it('approves the on-chain total plus the fee tolerance when the allowance only covers the quote', async () => {
    await expect(getManaApprovalNeeded(onChainTotal, () => Promise.resolve(1000n * ETHER))).resolves.toBe(padded)
  })

  it('asks for no approval once the allowance covers the padded on-chain total', async () => {
    await expect(getManaApprovalNeeded(onChainTotal, () => Promise.resolve(padded))).resolves.toBeNull()
  })

  it('treats a failed allowance read as no allowance', async () => {
    await expect(getManaApprovalNeeded(onChainTotal, () => Promise.reject(new Error('rpc down')))).resolves.toBe(padded)
  })

  it('neither reads nor approves a free fee', async () => {
    const read = vi.fn()
    await expect(getManaApprovalNeeded(0n, read)).resolves.toBeNull()
    expect(read).not.toHaveBeenCalled()
  })
})

describe('createCollection encoding', () => {
  it('builds the CollectionManager arguments from the collection and its items sorted by creation', () => {
    const args = buildCreateCollectionArgs(collection, items, ADDRESS, CHAIN_ID)
    expect(args[0]).toBe(getContract(ContractName.Forwarder, CHAIN_ID).address)
    expect(args[1]).toBe(getContract(ContractName.CollectionFactoryV3, CHAIN_ID).address)
    expect(args[2]).toBe(SALT)
    expect(args[3]).toBe('Halloween')
    expect(args[4]).toBe('DCL-HLLWN')
    expect(args[6]).toBe(ADDRESS)
    expect(args[7].map(item => item[3])).toEqual(['1:w:Item a::hat:BaseFemale', '1:w:Item b::hat:BaseFemale'])
    expect(args[7][0][0]).toBe('legendary')
  })

  it('refuses a collection the server has not given a salt to', () => {
    expect(() => buildCreateCollectionArgs({ ...collection, salt: undefined }, items, ADDRESS, CHAIN_ID)).toThrow(
      PublishCollectionError
    )
  })

  it('encodes the external call the credits server co-signs', () => {
    const args = buildCreateCollectionArgs(collection, items, ADDRESS, CHAIN_ID)
    const now = 1_700_000_000_000
    const call = buildCollectionManagerExternalCall(CHAIN_ID, args, now, '0x' + '22'.repeat(32))
    const manager = getContract(ContractName.CollectionManager, CHAIN_ID)
    expect(call.target).toBe(manager.address)
    expect(call.selector).toBe(
      toFunctionSelector(
        'createCollection(address,address,bytes32,string,string,string,address,(string,uint256,address,string)[])'
      )
    )
    expect(call.expiresAt).toBe(now / 1000 + 24 * 60 * 60)
    const decoded = decodeAbiParameters(
      parseAbiParameters(
        'address, address, bytes32, string, string, string, address, (string,uint256,address,string)[]'
      ),
      call.data as `0x${string}`
    )
    expect(decoded[3]).toBe('Halloween')
    expect(decoded[7]).toHaveLength(2)
    // Encoded by ethers before the move to viem: the bytes the credits server co-signs must not change.
    expect(call.data).toBe(
      '0x' +
        '0000000000000000000000007b1fe9de545b22cb553766817b84d655ce8121c9' +
        '000000000000000000000000802de0c509add2ee29de24de7225daaff4741c43' +
        '1111111111111111111111111111111111111111111111111111111111111111' +
        '0000000000000000000000000000000000000000000000000000000000000100' +
        '0000000000000000000000000000000000000000000000000000000000000140' +
        '0000000000000000000000000000000000000000000000000000000000000180' +
        '00000000000000000000000000000000000000000000000000000000000000aa' +
        '0000000000000000000000000000000000000000000000000000000000000200' +
        '0000000000000000000000000000000000000000000000000000000000000009' +
        '48616c6c6f7765656e0000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000009' +
        '44434c2d484c4c574e0000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000043' +
        '68747470733a2f2f706565722e646563656e7472616c616e642e7a6f6e652f6c' +
        '616d626461732f636f6c6c656374696f6e732f7374616e646172642f65726337' +
        '32312f0000000000000000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000002' +
        '0000000000000000000000000000000000000000000000000000000000000040' +
        '0000000000000000000000000000000000000000000000000000000000000140' +
        '0000000000000000000000000000000000000000000000000000000000000080' +
        'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff' +
        '00000000000000000000000000000000000000000000000000000000000000aa' +
        '00000000000000000000000000000000000000000000000000000000000000c0' +
        '0000000000000000000000000000000000000000000000000000000000000009' +
        '6c6567656e646172790000000000000000000000000000000000000000000000' +
        '000000000000000000000000000000000000000000000000000000000000001a' +
        '313a773a4974656d20613a3a6861743a4261736546656d616c65000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000080' +
        'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff' +
        '00000000000000000000000000000000000000000000000000000000000000aa' +
        '00000000000000000000000000000000000000000000000000000000000000c0' +
        '0000000000000000000000000000000000000000000000000000000000000009' +
        '6c6567656e646172790000000000000000000000000000000000000000000000' +
        '000000000000000000000000000000000000000000000000000000000000001a' +
        '313a773a4974656d20623a3a6861743a4261736546656d616c65000000000000'
    )
  })

  it('produces a useCredits call the CreditsManager ABI accepts, with no MANA from the wallet', () => {
    const args = buildCreateCollectionArgs(collection, items, ADDRESS, CHAIN_ID)
    const externalCall = buildCollectionManagerExternalCall(CHAIN_ID, args)
    const call = buildUseCreditsCall(CHAIN_ID, authorization, externalCall)
    expect(call.contract.address).toBe(getContract(ContractName.CreditsManager, CHAIN_ID).address)
    const [useCredits] = decodeCall(call) as any[]
    expect(useCredits.maxUncreditedValue).toBe(0n)
    expect(useCredits.maxCreditedValue).toBe(BigInt(authorization.credit.amount))
    expect(useCredits.customExternalCallSignature).toBe(authorization.externalCallSignature)
  })
})

describe('toPublishError', () => {
  it('maps known failures to their reasons and everything else to generic', () => {
    expect(toPublishError(new CreditsServerError('Insufficient credits', 402)).reason).toBe('insufficient_credits')
    expect(toPublishError(new BuilderServerError('locked', 423)).reason).toBe('locked')
    expect(toPublishError(Object.assign(new Error('denied'), { code: 4001 })).reason).toBe('rejected')
    expect(toPublishError(Object.assign(new Error('x'), { code: 'ACTION_REJECTED' })).reason).toBe('rejected')
    expect(toPublishError(new Error('MetaMask Tx Signature: User denied transaction signature.')).reason).toBe(
      'rejected'
    )
    expect(toPublishError(new Error('boom')).reason).toBe('generic')
    expect(toPublishError(new PublishCollectionError('unsynced')).reason).toBe('unsynced')
  })

  it('tells a wallet on another account or network apart, so the creator is asked to switch it back', () => {
    expect(toPublishError(new WrongAccountError('0xme', '0xother')).reason).toBe('wallet')
    expect(toPublishError(new WrongChainError(137, 1)).reason).toBe('wallet')
  })
})

describe('publishCollection', () => {
  const params = {
    address: ADDRESS,
    collection,
    items,
    paymentMethod: 'credits' as const,
    fee,
    email: 'jane.doe@example.com',
    chainId: CHAIN_ID
  }

  it('re-saves, verifies, records the ToS, pays with credits and locks — in that order', async () => {
    const deps = makeDeps()
    const result = await publishCollection(params, deps)
    expect(deps.calls).toEqual([
      'saveCollection',
      'fetchItems',
      'saveTOS',
      'authorizePublication',
      'sendTransaction',
      'lockCollection'
    ])
    expect(result.txHash).toBe('0xtx')
    expect(result.collection.lock).toBe(1234)
    expect(result.collection.contractAddress).toBe('0xc0ffee')
  })

  it('charges the fee in cents and sends the useCredits transaction to the CreditsManager', async () => {
    vi.useFakeTimers({ now: 1_700_000_000_000, toFake: ['Date'] })
    const authorize = vi.fn().mockResolvedValue(authorization)
    const send = vi.fn().mockResolvedValue('0xtx')
    const deps = makeDeps({ authorizePublication: authorize, sendTransaction: send, randomSalt: () => EXTERNAL_SALT })
    try {
      await publishCollection(params, deps)
    } finally {
      vi.useRealTimers()
    }
    expect(authorize).toHaveBeenCalledWith(
      expect.objectContaining({
        usdPriceCents: 1000,
        chainId: CHAIN_ID,
        creditsManagerAddress: getContract(ContractName.CreditsManager, CHAIN_ID).address
      })
    )
    // The credits server co-signs this exact call.
    expect(authorize.mock.calls[0][0].externalCall).toEqual({
      target: '0x5309ae874fc4eb21adcd63f8b6c3f766cc3b1849',
      selector: '0x35a629c2',
      data:
        '0x0000000000000000000000007b1fe9de545b22cb553766817b84d655ce8121c9' +
        '000000000000000000000000802de0c509add2ee29de24de7225daaff4741c43' +
        '1111111111111111111111111111111111111111111111111111111111111111' +
        '0000000000000000000000000000000000000000000000000000000000000100' +
        '0000000000000000000000000000000000000000000000000000000000000140' +
        '0000000000000000000000000000000000000000000000000000000000000180' +
        '00000000000000000000000000000000000000000000000000000000000000aa' +
        '0000000000000000000000000000000000000000000000000000000000000200' +
        '0000000000000000000000000000000000000000000000000000000000000009' +
        '48616c6c6f7765656e0000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000009' +
        '44434c2d484c4c574e0000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000043' +
        '68747470733a2f2f706565722e646563656e7472616c616e642e7a6f6e652f6c' +
        '616d626461732f636f6c6c656374696f6e732f7374616e646172642f65726337' +
        '32312f0000000000000000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000002' +
        '0000000000000000000000000000000000000000000000000000000000000040' +
        '0000000000000000000000000000000000000000000000000000000000000140' +
        '0000000000000000000000000000000000000000000000000000000000000080' +
        'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff' +
        '00000000000000000000000000000000000000000000000000000000000000aa' +
        '00000000000000000000000000000000000000000000000000000000000000c0' +
        '0000000000000000000000000000000000000000000000000000000000000009' +
        '6c6567656e646172790000000000000000000000000000000000000000000000' +
        '000000000000000000000000000000000000000000000000000000000000001a' +
        '313a773a4974656d20613a3a6861743a4261736546656d616c65000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000080' +
        'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff' +
        '00000000000000000000000000000000000000000000000000000000000000aa' +
        '00000000000000000000000000000000000000000000000000000000000000c0' +
        '0000000000000000000000000000000000000000000000000000000000000009' +
        '6c6567656e646172790000000000000000000000000000000000000000000000' +
        '000000000000000000000000000000000000000000000000000000000000001a' +
        '313a773a4974656d20623a3a6861743a4261736546656d616c65000000000000',
      expiresAt: 1700086400,
      salt: '0x2222222222222222222222222222222222222222222222222222222222222222'
    })
    expect(toTransaction(send.mock.calls[0][0])).toEqual({
      to: '0x8052a560e6e6ac86eeb7e711a4497f639b322fb3',
      data:
        '0x1863572d' +
        '0000000000000000000000000000000000000000000000000000000000000020' +
        '00000000000000000000000000000000000000000000000000000000000000c0' +
        '0000000000000000000000000000000000000000000000000000000000000140' +
        '0000000000000000000000000000000000000000000000000000000000000200' +
        '0000000000000000000000000000000000000000000000000000000000000720' +
        '0000000000000000000000000000000000000000000000000000000000000000' +
        '00000000000000000000000000000000000000000000003635c9adc5dea00000' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '00000000000000000000000000000000000000000000003635c9adc5dea00000' +
        '000000000000000000000000000000000000000000000000000000006553f100' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000020' +
        '0000000000000000000000000000000000000000000000000000000000000041' +
        'abababababababababababababababababababababababababababababababab' +
        'abababababababababababababababababababababababababababababababab' +
        'ab00000000000000000000000000000000000000000000000000000000000000' +
        '0000000000000000000000005309ae874fc4eb21adcd63f8b6c3f766cc3b1849' +
        '35a629c200000000000000000000000000000000000000000000000000000000' +
        '00000000000000000000000000000000000000000000000000000000000000a0' +
        '0000000000000000000000000000000000000000000000000000000065554280' +
        '2222222222222222222222222222222222222222222222222222222222222222' +
        '0000000000000000000000000000000000000000000000000000000000000460' +
        '0000000000000000000000007b1fe9de545b22cb553766817b84d655ce8121c9' +
        '000000000000000000000000802de0c509add2ee29de24de7225daaff4741c43' +
        '1111111111111111111111111111111111111111111111111111111111111111' +
        '0000000000000000000000000000000000000000000000000000000000000100' +
        '0000000000000000000000000000000000000000000000000000000000000140' +
        '0000000000000000000000000000000000000000000000000000000000000180' +
        '00000000000000000000000000000000000000000000000000000000000000aa' +
        '0000000000000000000000000000000000000000000000000000000000000200' +
        '0000000000000000000000000000000000000000000000000000000000000009' +
        '48616c6c6f7765656e0000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000009' +
        '44434c2d484c4c574e0000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000043' +
        '68747470733a2f2f706565722e646563656e7472616c616e642e7a6f6e652f6c' +
        '616d626461732f636f6c6c656374696f6e732f7374616e646172642f65726337' +
        '32312f0000000000000000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000002' +
        '0000000000000000000000000000000000000000000000000000000000000040' +
        '0000000000000000000000000000000000000000000000000000000000000140' +
        '0000000000000000000000000000000000000000000000000000000000000080' +
        'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff' +
        '00000000000000000000000000000000000000000000000000000000000000aa' +
        '00000000000000000000000000000000000000000000000000000000000000c0' +
        '0000000000000000000000000000000000000000000000000000000000000009' +
        '6c6567656e646172790000000000000000000000000000000000000000000000' +
        '000000000000000000000000000000000000000000000000000000000000001a' +
        '313a773a4974656d20613a3a6861743a4261736546656d616c65000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000080' +
        'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff' +
        '00000000000000000000000000000000000000000000000000000000000000aa' +
        '00000000000000000000000000000000000000000000000000000000000000c0' +
        '0000000000000000000000000000000000000000000000000000000000000009' +
        '6c6567656e646172790000000000000000000000000000000000000000000000' +
        '000000000000000000000000000000000000000000000000000000000000001a' +
        '313a773a4974656d20623a3a6861743a4261736546656d616c65000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000041' +
        'cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd' +
        'cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd' +
        'cd00000000000000000000000000000000000000000000000000000000000000'
    })
  })

  it('pays with MANA by calling createCollection directly', async () => {
    const send = vi.fn().mockResolvedValue('0xtx')
    const deps = makeDeps({ sendTransaction: send })
    await publishCollection({ ...params, paymentMethod: 'mana' }, deps)
    expect(deps.calls).not.toContain('authorizePublication')
    expect(toTransaction(send.mock.calls[0][0])).toEqual({
      to: '0x5309ae874fc4eb21adcd63f8b6c3f766cc3b1849',
      data:
        '0x35a629c2' +
        '0000000000000000000000007b1fe9de545b22cb553766817b84d655ce8121c9' +
        '000000000000000000000000802de0c509add2ee29de24de7225daaff4741c43' +
        '1111111111111111111111111111111111111111111111111111111111111111' +
        '0000000000000000000000000000000000000000000000000000000000000100' +
        '0000000000000000000000000000000000000000000000000000000000000140' +
        '0000000000000000000000000000000000000000000000000000000000000180' +
        '00000000000000000000000000000000000000000000000000000000000000aa' +
        '0000000000000000000000000000000000000000000000000000000000000200' +
        '0000000000000000000000000000000000000000000000000000000000000009' +
        '48616c6c6f7765656e0000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000009' +
        '44434c2d484c4c574e0000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000043' +
        '68747470733a2f2f706565722e646563656e7472616c616e642e7a6f6e652f6c' +
        '616d626461732f636f6c6c656374696f6e732f7374616e646172642f65726337' +
        '32312f0000000000000000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000002' +
        '0000000000000000000000000000000000000000000000000000000000000040' +
        '0000000000000000000000000000000000000000000000000000000000000140' +
        '0000000000000000000000000000000000000000000000000000000000000080' +
        'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff' +
        '00000000000000000000000000000000000000000000000000000000000000aa' +
        '00000000000000000000000000000000000000000000000000000000000000c0' +
        '0000000000000000000000000000000000000000000000000000000000000009' +
        '6c6567656e646172790000000000000000000000000000000000000000000000' +
        '000000000000000000000000000000000000000000000000000000000000001a' +
        '313a773a4974656d20613a3a6861743a4261736546656d616c65000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000080' +
        'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff' +
        '00000000000000000000000000000000000000000000000000000000000000aa' +
        '00000000000000000000000000000000000000000000000000000000000000c0' +
        '0000000000000000000000000000000000000000000000000000000000000009' +
        '6c6567656e646172790000000000000000000000000000000000000000000000' +
        '000000000000000000000000000000000000000000000000000000000000001a' +
        '313a773a4974656d20623a3a6861743a4261736546656d616c65000000000000'
    })
  })

  it('skips the re-save while the collection is locked', async () => {
    const deps = makeDeps()
    await publishCollection({ ...params, collection: { ...collection, lock: Date.now() } }, deps)
    expect(deps.calls).not.toContain('saveCollection')
    expect(deps.calls).toContain('saveTOS')
  })

  it('never pays when the ToS acceptance cannot be recorded, after retrying a server failure', async () => {
    const saveTOS = vi.fn().mockRejectedValue(new BuilderServerError('warehouse down', 500))
    const deps = makeDeps({ saveTOS })
    await expect(publishCollection(params, deps)).rejects.toMatchObject({ reason: 'tos_failed' })
    expect(saveTOS).toHaveBeenCalledTimes(3)
    expect(deps.calls).not.toContain('sendTransaction')
    expect(deps.calls).not.toContain('lockCollection')
  })

  it('does not retry a ToS record the server rejected', async () => {
    const saveTOS = vi.fn().mockRejectedValue(new BuilderServerError('invalid email', 400))
    await expect(publishCollection(params, makeDeps({ saveTOS }))).rejects.toMatchObject({ reason: 'tos_failed' })
    expect(saveTOS).toHaveBeenCalledTimes(1)
  })

  it('re-saves items still carrying legacy hashes before the ToS and the payment', async () => {
    const legacy = { ...items[0], contents: { 'm.glb': 'QmOld', 'thumbnail.png': 'thumb' } }
    const rehashed = { ...legacy, contents: { 'm.glb': 'bafnew', 'thumbnail.png': 'thumb' } }
    const rehash = vi.fn().mockResolvedValue(rehashed)
    const deps = makeDeps({ fetchItems: async () => [legacy, items[1]], rehashItem: rehash })
    await publishCollection({ ...params, items: [legacy, items[1]] }, deps)
    expect(rehash).toHaveBeenCalledTimes(1)
    expect(rehash).toHaveBeenCalledWith(legacy)
    expect(deps.calls.indexOf('rehashItem')).toBeLessThan(deps.calls.indexOf('saveTOS'))
  })

  it('leaves items with current hashes alone', async () => {
    const deps = makeDeps()
    await publishCollection(params, deps)
    expect(deps.calls).not.toContain('rehashItem')
  })

  it('aborts before paying when the server items differ from the reviewed ones', async () => {
    const deps = makeDeps({ fetchItems: async () => [items[0]] })
    await expect(publishCollection(params, deps)).rejects.toMatchObject({ reason: 'unsynced' })
    expect(deps.calls).not.toContain('sendTransaction')
  })

  it('still succeeds when the lock fails after the transaction was sent, so it is never paid twice', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {})
    const deps = makeDeps({ lockCollection: async () => Promise.reject(new Error('boom')) })
    const result = await publishCollection(params, deps)
    expect(result.txHash).toBe('0xtx')
    expect(result.collection.lock).toBe(collection.lock)
  })

  it('fails when the server never produced a salt', async () => {
    const deps = makeDeps({ saveCollection: async c => ({ ...c, salt: undefined }) })
    await expect(publishCollection(params, deps)).rejects.toMatchObject({ reason: 'missing_salt' })
  })

  it('normalizes payment failures', async () => {
    const deps = makeDeps({ authorizePublication: async () => Promise.reject(new CreditsServerError('no', 402)) })
    await expect(publishCollection(params, deps)).rejects.toMatchObject({ reason: 'insufficient_credits' })
  })
})

describe('syncPublishedItems', () => {
  it('polls for an hour at most while the graph lags, by default', async () => {
    vi.useFakeTimers()
    const publish = vi.fn().mockRejectedValue(new BuilderServerError('not yet', 401))
    const run = syncPublishedItems('col-1', { publishCollectionItems: publish })
    const failure = expect(run).rejects.toThrow('not yet')
    await vi.advanceTimersByTimeAsync(60 * 60 * 1000)
    await failure
    expect(publish).toHaveBeenCalledTimes(721)
    vi.useRealTimers()
  })
})

describe('isRecentPublication', () => {
  const now = Date.UTC(2026, 10, 5)
  const day = 24 * 60 * 60_000

  it('counts a collection published this month', () => {
    expect(isRecentPublication({ lock: now - 3 * day, updatedAt: now - 10 * day }, now)).toBe(true)
  })

  it('leaves out a collection published over a month ago', () => {
    expect(isRecentPublication({ lock: undefined, updatedAt: now - 45 * day }, now)).toBe(false)
  })
})

describe('openFirstReviewRequest', () => {
  const pending = { collectionId: 'col-1', status: 'pending' }

  it('opens the request when the collection has none and hands back the pending curation', async () => {
    const pushCollectionCuration = vi.fn().mockResolvedValue(pending)
    const fetchCollectionCuration = vi.fn().mockResolvedValue(null)
    await expect(openFirstReviewRequest('col-1', { fetchCollectionCuration, pushCollectionCuration })).resolves.toBe(
      pending
    )
    expect(pushCollectionCuration).toHaveBeenCalledWith('col-1')
  })

  it('keeps a request that already exists, so a curator decision is never reopened', async () => {
    const rejected = { collectionId: 'col-1', status: 'rejected' }
    const pushCollectionCuration = vi.fn()
    const fetchCollectionCuration = vi.fn().mockResolvedValue(rejected)
    await expect(openFirstReviewRequest('col-1', { fetchCollectionCuration, pushCollectionCuration })).resolves.toBe(
      rejected
    )
    expect(pushCollectionCuration).not.toHaveBeenCalled()
  })

  it('hands back the request another tab opened in the meantime', async () => {
    const pushCollectionCuration = vi
      .fn()
      .mockRejectedValue(new BuilderServerError('There is already an ongoing review request', 400))
    const fetchCollectionCuration = vi.fn().mockResolvedValueOnce(null).mockResolvedValueOnce(pending)
    await expect(openFirstReviewRequest('col-1', { fetchCollectionCuration, pushCollectionCuration })).resolves.toBe(
      pending
    )
  })

  it('surfaces any other failure', async () => {
    const pushCollectionCuration = vi.fn().mockRejectedValue(new BuilderServerError('Unauthorized', 401))
    const fetchCollectionCuration = vi.fn().mockResolvedValue(null)
    await expect(
      openFirstReviewRequest('col-1', { fetchCollectionCuration, pushCollectionCuration })
    ).rejects.toBeInstanceOf(BuilderServerError)
  })
})

describe('consolidatePublishedCollection', () => {
  it('runs onSynced only once the server holds the published items', async () => {
    const onSynced = vi.fn()
    await consolidatePublishedCollection(
      'col-1',
      '0xtx',
      { waitForTransaction: async () => true, publishCollectionItems: vi.fn().mockResolvedValue({}), onSynced },
      0,
      0
    )
    expect(onSynced).toHaveBeenCalledTimes(1)

    const notSynced = vi.fn()
    await expect(
      consolidatePublishedCollection(
        'col-1',
        '0xtx',
        { waitForTransaction: async () => false, publishCollectionItems: vi.fn(), onSynced: notSynced },
        0,
        0
      )
    ).rejects.toBeInstanceOf(PublishTransactionRevertedError)
    await expect(
      consolidatePublishedCollection(
        'col-1',
        '0xtx',
        {
          waitForTransaction: async () => true,
          publishCollectionItems: async () => Promise.reject(new BuilderServerError('x', 500)),
          onSynced: notSynced
        },
        0,
        0
      )
    ).rejects.toBeInstanceOf(BuilderServerError)
    expect(notSynced).not.toHaveBeenCalled()
  })

  it('waits for the transaction, then retries the server sync while the graph lags', async () => {
    const publish = vi.fn().mockRejectedValueOnce(new BuilderServerError('not yet', 401)).mockResolvedValueOnce({})
    await consolidatePublishedCollection(
      'col-1',
      '0xtx',
      { waitForTransaction: async () => true, publishCollectionItems: publish },
      3,
      0
    )
    expect(publish).toHaveBeenCalledTimes(2)
  })

  it('syncs a retry that reverted because an earlier attempt already deployed the collection', async () => {
    const onSynced = vi.fn()
    await consolidatePublishedCollection(
      'col-1',
      '0xretry',
      {
        waitForTransaction: async () => false,
        isCollectionDeployed: async () => true,
        publishCollectionItems: vi.fn().mockResolvedValue({}),
        onSynced
      },
      0,
      0
    )
    expect(onSynced).toHaveBeenCalledTimes(1)
  })

  it('reports a revert when the collection is not on chain or that cannot be read', async () => {
    for (const isCollectionDeployed of [async () => false, async () => Promise.reject(new Error('rpc down'))]) {
      await expect(
        consolidatePublishedCollection(
          'col-1',
          '0xtx',
          { waitForTransaction: async () => false, isCollectionDeployed, publishCollectionItems: vi.fn() },
          0,
          0
        )
      ).rejects.toBeInstanceOf(PublishTransactionRevertedError)
    }
  })

  it('gives up on a reverted transaction or a non-401 failure', async () => {
    await expect(
      consolidatePublishedCollection(
        'col-1',
        '0xtx',
        { waitForTransaction: async () => false, publishCollectionItems: vi.fn() },
        3,
        0
      )
    ).rejects.toBeInstanceOf(PublishTransactionRevertedError)
    await expect(
      consolidatePublishedCollection(
        'col-1',
        '0xtx',
        {
          waitForTransaction: async () => true,
          publishCollectionItems: async () => Promise.reject(new BuilderServerError('x', 500))
        },
        3,
        0
      )
    ).rejects.toThrow('x')
  })
})
