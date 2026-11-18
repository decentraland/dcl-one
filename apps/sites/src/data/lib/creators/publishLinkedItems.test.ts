import { describe, expect, it, vi } from 'vitest'
import { type ContractNetwork } from '@dcl/schemas'
import {
  TEST_SIGNER,
  decodeCall,
  recoverTypedDataSigner,
  signTypedDataWithTestKey,
  typedDataDigest
} from '~/test/chain'
import { BuilderServerError, type SlotsCheque } from './builder'
import { CurationStatus, type Collection } from './collections'
import { ItemType, type Item } from './items'
import { type ItemCuration } from './linkedItemStatus'
import {
  LinkedPublishError,
  buildChequeTypedData,
  createChequeSalt,
  getLinkedPublishFee,
  getLinkedPublishPhase,
  getLinkedPublishPhases,
  getLinkedSlots,
  getResumablePayment,
  getSlotsToBuy,
  readFreshLinkedQuote,
  publishLinkedItems,
  signSlotsCheque,
  toLinkedPublishError,
  type LinkedPublishStep,
  type PublishLinkedDeps,
  type PublishLinkedParams
} from './publishLinkedItems'
import { type ThirdParty } from './thirdParty'
import { getThirdPartyRegistry, type ThirdPartyPrice } from './thirdPartyRegistry'
import { WrongAccountError } from './walletErrors'

const CHAIN_ID = 80002
const ADDRESS = '0x00000000000000000000000000000000000000aa'
const ETHER = 10n ** 18n
const THIRD_PARTY_ID = 'urn:decentraland:amoy:collections-thirdparty:brand'
const REGISTRY = getThirdPartyRegistry(CHAIN_ID)

const collection: Collection = {
  id: 'col-1',
  name: 'Brand drop',
  owner: '',
  urn: `${THIRD_PARTY_ID}:col-1`,
  isPublished: false,
  isApproved: false,
  itemCount: 3,
  minters: [],
  managers: [],
  linkedContractAddress: '0x' + '1'.repeat(40),
  linkedContractNetwork: 'amoy',
  createdAt: 1,
  updatedAt: 1
}

const thirdParty: ThirdParty = {
  id: THIRD_PARTY_ID,
  root: '',
  managers: [ADDRESS],
  name: 'Brand',
  description: 'A brand',
  isApproved: false,
  isProgrammatic: false,
  maxItems: '0',
  contracts: [{ network: 'amoy' as ContractNetwork, address: '0x' + '1'.repeat(40) }],
  published: false
}

// 100 USD per slot at 0.5 USD per MANA: 200 MANA per slot; programmatic pack of 200 slots.
const price: ThirdPartyPrice = {
  itemSlotUsdWei: 100n * ETHER,
  itemSlotManaWei: 200n * ETHER,
  programmaticMinSlots: 200,
  usdPerManaWei: ETHER / 2n
}

const cheque: SlotsCheque = { signature: '0x' + 'ab'.repeat(65), qty: 3, salt: '0x' + '11'.repeat(32) }

function makeItem(id: string, overrides: Partial<Item> = {}): Item {
  return {
    id,
    name: `Item ${id}`,
    description: '',
    thumbnail: 'thumbnail.png',
    owner: ADDRESS,
    collectionId: collection.id,
    isPublished: false,
    isApproved: false,
    inCatalyst: false,
    currentContentHash: `hash-${id}`,
    type: ItemType.WEARABLE,
    data: { category: 'hat', representations: [] },
    contents: {},
    createdAt: 1,
    updatedAt: 1,
    ...overrides
  }
}

const itemsToPublish = [makeItem('a'), makeItem('b'), makeItem('c')]
const itemsToPush = [makeItem('p', { isPublished: true, isApproved: true, inCatalyst: true })]

function pendingCuration(item: Item): ItemCuration {
  return {
    id: `ic-${item.id}`,
    itemId: item.id,
    status: CurationStatus.PENDING,
    contentHash: item.currentContentHash ?? '',
    isMappingComplete: true,
    createdAt: 1,
    updatedAt: 1
  }
}

const publishedResult = {
  collection: { ...collection, isPublished: true },
  items: itemsToPublish.map(item => ({ ...item, isPublished: true })),
  itemCurations: itemsToPublish.map(pendingCuration)
}

function makeParams(overrides: Partial<PublishLinkedParams> = {}): PublishLinkedParams {
  return {
    address: ADDRESS,
    chainId: CHAIN_ID,
    collection,
    thirdParty,
    itemsToPublish,
    itemsToPush: [],
    cheque,
    email: 'jane.doe@example.com',
    isProgrammatic: false,
    slots: { available: 0, registered: false },
    price,
    ...overrides
  }
}

type Deps = { [K in keyof Required<PublishLinkedDeps>]: ReturnType<typeof vi.fn> } & PublishLinkedDeps

function makeDeps(overrides: Partial<PublishLinkedDeps> = {}): Deps {
  let txCount = 0
  return {
    // The item to push is approved with older content, as the list showed it.
    fetchCurations: vi.fn(
      async () =>
        new Map<string, ItemCuration>([
          ['p', { ...pendingCuration(itemsToPush[0]), status: CurationStatus.APPROVED, contentHash: 'old' }]
        ])
    ),
    saveTOS: vi.fn(async () => {}),
    setThirdPartyKind: vi.fn(async () => {}),
    fetchManaAllowance: vi.fn(async () => 0n),
    sendTransaction: vi.fn(async () => `0xtx${++txCount}`),
    settleTransaction: vi.fn(async () => 'mined' as const),
    fetchSlots: vi.fn(async () => ({ available: 0, registered: true })),
    deleteVirtualThirdParty: vi.fn(async () => {}),
    publishLinkedItems: vi.fn(async () => publishedResult),
    pushChanges: vi.fn(async (items: Item[]) => ({ pushed: items, failed: [] })),
    onStep: vi.fn(),
    sleep: vi.fn(async () => {}),
    ...overrides
  } as Deps
}

function sentMethods(deps: Deps): string[] {
  return deps.sendTransaction.mock.calls.map(([call]) => `${call.method}`)
}

function decodeSent(deps: Deps, index: number): any[] {
  const [call] = deps.sendTransaction.mock.calls[index]
  return decodeCall(call) as any[]
}

function steps(deps: Deps): LinkedPublishStep[] {
  return deps.onStep.mock.calls.map(([step]) => step as LinkedPublishStep)
}

function serverError(status: number, message: string) {
  return new BuilderServerError(message, status)
}

const INSUFFICIENT_SLOTS = serverError(409, 'The amount of items to publish exceeds the available slots.')
const PENDING_REVIEW = serverError(409, "The third party collection already has published items. It can't be updated.")

async function failure(promise: Promise<unknown>): Promise<LinkedPublishError> {
  try {
    await promise
  } catch (error) {
    expect(error).toBeInstanceOf(LinkedPublishError)
    return error as LinkedPublishError
  }
  throw new Error('Expected the publish to fail')
}

describe('buildChequeTypedData', () => {
  it('builds the ConsumeSlots message with the chain id as the domain salt, as builder-server verifies it', () => {
    const salt = '0x' + '22'.repeat(32)
    const typed = buildChequeTypedData(CHAIN_ID, THIRD_PARTY_ID, 3, salt)

    expect(typed.domain).toEqual({
      name: 'Decentraland Third Party Registry',
      version: '1',
      verifyingContract: REGISTRY.address,
      salt: '0x' + '0'.repeat(59) + '13882'
    })
    expect(Object.keys(typed.types)).toEqual(['ConsumeSlots'])
    expect(typed.types.ConsumeSlots).toEqual([
      { name: 'thirdPartyId', type: 'string' },
      { name: 'qty', type: 'uint256' },
      { name: 'salt', type: 'bytes32' }
    ])
    expect(typed.value).toEqual({ thirdPartyId: THIRD_PARTY_ID, qty: 3, salt })
    expect(typedDataDigest(typed.domain, typed.types, typed.value)).toBe(
      '0xa96ab1f5e7bde3bee11f939568e08b49d8fcdc039b990c25f672be61acd8c97b'
    )
  })
})

describe('createChequeSalt', () => {
  it('answers a fresh 32-byte hex string every time', () => {
    const first = createChequeSalt()
    const second = createChequeSalt()
    expect(first).toMatch(/^0x[0-9a-f]{64}$/)
    expect(second).toMatch(/^0x[0-9a-f]{64}$/)
    expect(first).not.toBe(second)
  })
})

describe('signSlotsCheque', () => {
  it('signs the ConsumeSlots cheque with the given salt, byte for byte', async () => {
    const salt = '0x' + '33'.repeat(32)
    const signed = await signSlotsCheque(
      { signTypedData: signTypedDataWithTestKey, randomSalt: () => salt },
      CHAIN_ID,
      THIRD_PARTY_ID,
      5
    )

    expect(signed).toEqual({
      qty: 5,
      salt,
      signature:
        '0x4fcfe12a7f648caab7cfde9184b5f28a6a7130f8111c67a71158e8f241eb1e19204092bbd56a0db39c11dcc5340b59bfd5bb1fc32732da56f7ff147b6dffdfc21b'
    })
  })

  it('signs the typed data and answers a cheque builder-server can recover the signer from', async () => {
    const signed = await signSlotsCheque({ signTypedData: signTypedDataWithTestKey }, CHAIN_ID, THIRD_PARTY_ID, 5)

    expect(signed.qty).toBe(5)
    expect(signed.salt).toMatch(/^0x[0-9a-f]{64}$/)
    const { domain, types, value } = buildChequeTypedData(CHAIN_ID, THIRD_PARTY_ID, signed.qty, signed.salt)
    expect(await recoverTypedDataSigner(domain, types, value, signed.signature)).toBe(TEST_SIGNER)
  })
})

describe('getSlotsToBuy', () => {
  const base = { isProgrammatic: false, isPublished: false, programmaticMinSlots: 200 }

  it('buys what the items exceed the free slots by for a standard third party', () => {
    expect(getSlotsToBuy({ ...base, itemsToPublish: 5, availableSlots: 2 })).toBe(3)
    expect(getSlotsToBuy({ ...base, itemsToPublish: 2, availableSlots: 5 })).toBe(0)
    expect(getSlotsToBuy({ ...base, itemsToPublish: 5, availableSlots: 2, isPublished: true })).toBe(3)
  })

  it('buys at least the base pack on a programmatic first publish', () => {
    expect(getSlotsToBuy({ ...base, isProgrammatic: true, itemsToPublish: 3, availableSlots: 0 })).toBe(200)
    expect(getSlotsToBuy({ ...base, isProgrammatic: true, itemsToPublish: 250, availableSlots: 0 })).toBe(250)
  })

  it('still buys the missing slots for a programmatic third party already on chain', () => {
    expect(
      getSlotsToBuy({ ...base, isProgrammatic: true, isPublished: true, itemsToPublish: 500, availableSlots: 20 })
    ).toBe(480)
    expect(
      getSlotsToBuy({ ...base, isProgrammatic: true, isPublished: true, itemsToPublish: 5, availableSlots: 20 })
    ).toBe(0)
  })
})

describe('getLinkedPublishFee', () => {
  it('multiplies the slot price and pads the MANA cap by 0.5%', () => {
    expect(getLinkedPublishFee(3, price)).toEqual({
      usdWei: 300n * ETHER,
      manaWei: 600n * ETHER,
      maxManaWei: 603n * ETHER
    })
  })

  it('charges a programmatic registration the base pack only, and nothing once it is on chain', () => {
    expect(getLinkedPublishFee(250, price, { isProgrammatic: true, isPublished: false })).toEqual(
      getLinkedPublishFee(200, price)
    )
    expect(getLinkedPublishFee(30, price, { isProgrammatic: true, isPublished: true })).toEqual({
      usdWei: 0n,
      manaWei: 0n,
      maxManaWei: 0n
    })
  })

  it('rounds the cap up and is zero for no slots', () => {
    const tiny: ThirdPartyPrice = { ...price, itemSlotManaWei: 1n }
    expect(getLinkedPublishFee(1, tiny).maxManaWei).toBe(2n)
    expect(getLinkedPublishFee(0, price)).toEqual({ usdWei: 0n, manaWei: 0n, maxManaWei: 0n })
  })
})

describe('publishLinkedItems', () => {
  it('registers a virtual third party, removes its virtual record and publishes (first publish)', async () => {
    const deps = makeDeps()

    const result = await publishLinkedItems(makeParams(), deps)

    expect(deps.saveTOS).toHaveBeenCalledWith(collection, 'jane.doe@example.com', ['hash-a', 'hash-b', 'hash-c'])
    expect(deps.setThirdPartyKind).not.toHaveBeenCalled()
    expect(sentMethods(deps)).toEqual(['approve', 'addThirdParties'])
    const approve = decodeSent(deps, 0)
    expect(approve[0].toLowerCase()).toBe(REGISTRY.address.toLowerCase())
    expect(approve[1]).toBe(603n * ETHER)
    const add = decodeSent(deps, 1)
    const [registered] = add[0]
    expect(registered.id).toBe(THIRD_PARTY_ID)
    expect(registered.metadata).toBe(`tp:1:Brand:A brand:amoy-${'0x' + '1'.repeat(40)}`)
    expect(registered.managers.map((manager: string) => manager.toLowerCase())).toEqual([ADDRESS])
    expect(registered.managerValues).toEqual([true])
    expect(registered.slots).toBe(3n)
    expect(add[1]).toEqual([false])
    expect(add[2]).toEqual([603n * ETHER])
    expect(deps.settleTransaction.mock.calls.map(([hash]) => hash)).toEqual(['0xtx1', '0xtx2'])
    expect(deps.deleteVirtualThirdParty).toHaveBeenCalledWith(THIRD_PARTY_ID)
    expect(deps.publishLinkedItems).toHaveBeenCalledWith(collection.id, ['a', 'b', 'c'], cheque)
    expect(deps.pushChanges).not.toHaveBeenCalled()
    expect(result).toEqual({
      collection: publishedResult.collection,
      items: publishedResult.items,
      itemCurations: publishedResult.itemCurations,
      pushed: null,
      txHash: '0xtx2'
    })
    expect(steps(deps)).toEqual(['check', 'tos', 'allowance', 'register', 'wait_tx', 'delete_virtual', 'publish'])
  })

  it('sets the programmatic kind first and registers with the base pack', async () => {
    const deps = makeDeps({ fetchManaAllowance: vi.fn(async () => 10n ** 30n) })

    await publishLinkedItems(makeParams({ isProgrammatic: true }), deps)

    expect(deps.setThirdPartyKind).toHaveBeenCalledWith(THIRD_PARTY_ID, true)
    expect(sentMethods(deps)).toEqual(['addThirdParties'])
    const add = decodeSent(deps, 0)
    expect(add[0][0].slots).toBe(200n)
    expect(add[1]).toEqual([true])
    expect(add[2][0]).toBe(getLinkedPublishFee(200, price).maxManaWei)
    expect(steps(deps)).toEqual([
      'check',
      'tos',
      'kind',
      'allowance',
      'register',
      'wait_tx',
      'delete_virtual',
      'publish'
    ])
  })

  it('buys only the missing slots for a third party already on chain, without approving when allowed', async () => {
    const deps = makeDeps({ fetchManaAllowance: vi.fn(async () => 10n ** 30n) })

    const result = await publishLinkedItems(
      makeParams({ thirdParty: { ...thirdParty, published: true }, slots: { available: 1, registered: true } }),
      deps
    )

    expect(deps.fetchManaAllowance).toHaveBeenCalledWith(ADDRESS, REGISTRY.address)
    expect(sentMethods(deps)).toEqual(['buyItemSlots'])
    const buy = decodeSent(deps, 0)
    expect(buy[0]).toBe(THIRD_PARTY_ID)
    expect(buy[1]).toBe(2n)
    expect(buy[2]).toBe(getLinkedPublishFee(2, price).maxManaWei)
    expect(deps.setThirdPartyKind).not.toHaveBeenCalled()
    expect(deps.deleteVirtualThirdParty).not.toHaveBeenCalled()
    expect(result.txHash).toBe('0xtx1')
    expect(steps(deps)).toEqual(['check', 'tos', 'allowance', 'buy_slots', 'wait_tx', 'publish'])
  })

  it('sends no transaction when the free slots cover the items', async () => {
    const deps = makeDeps()

    const result = await publishLinkedItems(
      makeParams({ thirdParty: { ...thirdParty, published: true }, slots: { available: 3, registered: true } }),
      deps
    )

    expect(deps.fetchManaAllowance).not.toHaveBeenCalled()
    expect(deps.sendTransaction).not.toHaveBeenCalled()
    expect(deps.deleteVirtualThirdParty).not.toHaveBeenCalled()
    expect(deps.publishLinkedItems).toHaveBeenCalledWith(collection.id, ['a', 'b', 'c'], cheque)
    expect(result.txHash).toBeNull()
    expect(steps(deps)).toEqual(['check', 'tos', 'publish'])
  })

  it('only pushes changes when there is nothing to publish', async () => {
    const deps = makeDeps()

    const result = await publishLinkedItems(
      makeParams({ thirdParty: { ...thirdParty, published: true }, itemsToPublish: [], itemsToPush, cheque: null }),
      deps
    )

    expect(deps.saveTOS).toHaveBeenCalledWith(collection, 'jane.doe@example.com', ['hash-p'])
    expect(deps.sendTransaction).not.toHaveBeenCalled()
    expect(deps.publishLinkedItems).not.toHaveBeenCalled()
    expect(deps.pushChanges).toHaveBeenCalledWith(itemsToPush, expect.any(Map))
    expect(result).toEqual({
      collection: undefined,
      items: [],
      itemCurations: [],
      pushed: { pushed: itemsToPush, failed: [] },
      txHash: null
    })
    expect(steps(deps)).toEqual(['tos', 'push'])
  })

  it('publishes before pushing and records both sets of hashes in the ToS', async () => {
    const order: string[] = []
    const deps = makeDeps({
      publishLinkedItems: vi.fn(async () => {
        order.push('publish')
        return publishedResult
      }),
      pushChanges: vi.fn(async (items: Item[]) => {
        order.push('push')
        return { pushed: items, failed: [] }
      })
    })

    const result = await publishLinkedItems(
      makeParams({
        thirdParty: { ...thirdParty, published: true },
        slots: { available: 3, registered: true },
        itemsToPush
      }),
      deps
    )

    expect(deps.saveTOS).toHaveBeenCalledWith(collection, 'jane.doe@example.com', [
      'hash-a',
      'hash-b',
      'hash-c',
      'hash-p'
    ])
    expect(order).toEqual(['publish', 'push'])
    expect(result.pushed).toEqual({ pushed: itemsToPush, failed: [] })
    expect(result.items).toEqual(publishedResult.items)
  })

  it('keeps removing the virtual third party while the server has not indexed the registration', async () => {
    const deleteVirtualThirdParty = vi
      .fn<(id: string) => Promise<void>>()
      .mockRejectedValueOnce(serverError(400, 'Third party not found on chain'))
      .mockRejectedValueOnce(serverError(400, 'Third party not found on chain'))
      .mockResolvedValueOnce(undefined)
    const deps = makeDeps({ deleteVirtualThirdParty })

    const result = await publishLinkedItems(makeParams(), deps)

    expect(deleteVirtualThirdParty).toHaveBeenCalledTimes(3)
    expect(deps.sleep).toHaveBeenCalledTimes(2)
    expect(result.items).toEqual(publishedResult.items)
  })

  it('treats an already removed virtual third party as done', async () => {
    const deps = makeDeps({
      deleteVirtualThirdParty: vi.fn(async () => {
        throw serverError(404, 'Not found')
      })
    })

    const result = await publishLinkedItems(makeParams(), deps)

    expect(result.items).toEqual(publishedResult.items)
  })

  it('retries the publish while the bought slots are not indexed yet', async () => {
    const publish = vi
      .fn<PublishLinkedDeps['publishLinkedItems']>()
      .mockRejectedValueOnce(INSUFFICIENT_SLOTS)
      .mockRejectedValueOnce(serverError(401, 'Unauthorized'))
      .mockResolvedValueOnce(publishedResult)
    const deps = makeDeps({ publishLinkedItems: publish })

    const result = await publishLinkedItems(makeParams(), deps)

    expect(publish).toHaveBeenCalledTimes(3)
    expect(result.itemCurations).toEqual(publishedResult.itemCurations)
  })

  it('counts a pending-review conflict after a server failure as a publish that went through', async () => {
    const publish = vi
      .fn<PublishLinkedDeps['publishLinkedItems']>()
      .mockRejectedValueOnce(serverError(500, 'Forum post failed'))
      .mockRejectedValueOnce(PENDING_REVIEW)
    const deps = makeDeps({ publishLinkedItems: publish })

    const result = await publishLinkedItems(
      makeParams({
        thirdParty: { ...thirdParty, published: true },
        slots: { available: 3, registered: true },
        itemsToPush
      }),
      deps
    )

    expect(publish).toHaveBeenCalledTimes(2)
    expect(result.collection).toBeUndefined()
    expect(result.items).toEqual([])
    expect(result.itemCurations).toEqual([])
    expect(deps.pushChanges).toHaveBeenCalledWith(itemsToPush, expect.any(Map))
  })

  it('fails with pending_review when the collection already has a review pending', async () => {
    const deps = makeDeps({
      publishLinkedItems: vi.fn(async () => {
        throw PENDING_REVIEW
      })
    })

    const error = await failure(
      publishLinkedItems(
        makeParams({ thirdParty: { ...thirdParty, published: true }, slots: { available: 3, registered: true } }),
        deps
      )
    )

    expect(error.step).toBe('publish')
    expect(error.reason).toBe('pending_review')
    expect(deps.publishLinkedItems).toHaveBeenCalledTimes(1)
  })

  it('gives up on insufficient slots once the retries run out', async () => {
    const deps = makeDeps({
      publishLinkedItems: vi.fn(async () => {
        throw INSUFFICIENT_SLOTS
      })
    })

    const error = await failure(publishLinkedItems(makeParams(), deps))

    expect(error.step).toBe('publish')
    expect(error.reason).toBe('insufficient_slots')
    expect(deps.publishLinkedItems).toHaveBeenCalledTimes(20)
  })

  it('reports a dismissed wallet prompt at the step it happened', async () => {
    const deps = makeDeps({
      sendTransaction: vi.fn(async () => {
        throw Object.assign(new Error('User rejected the request'), { code: 4001 })
      }),
      fetchManaAllowance: vi.fn(async () => 10n ** 30n)
    })

    const error = await failure(publishLinkedItems(makeParams(), deps))

    expect(error.step).toBe('register')
    expect(error.reason).toBe('rejected')
    expect(deps.deleteVirtualThirdParty).not.toHaveBeenCalled()
    expect(deps.publishLinkedItems).not.toHaveBeenCalled()
  })

  it('reports a reverted purchase with its transaction hash', async () => {
    const deps = makeDeps({
      fetchManaAllowance: vi.fn(async () => 10n ** 30n),
      settleTransaction: vi.fn(async () => 'reverted' as const)
    })

    const error = await failure(
      publishLinkedItems(makeParams({ thirdParty: { ...thirdParty, published: true } }), deps)
    )

    expect(error.step).toBe('wait_tx')
    expect(error.reason).toBe('reverted')
    expect(error.txHash).toBe('0xtx1')
    expect(deps.publishLinkedItems).not.toHaveBeenCalled()
  })

  it('buys the missing slots of a programmatic third party on chain for free, without a MANA approval', async () => {
    const deps = makeDeps()

    await publishLinkedItems(
      makeParams({ thirdParty: { ...thirdParty, published: true, isProgrammatic: true }, isProgrammatic: true }),
      deps
    )

    expect(deps.fetchManaAllowance).not.toHaveBeenCalled()
    expect(sentMethods(deps)).toEqual(['buyItemSlots'])
    const buy = decodeSent(deps, 0)
    expect(buy[1]).toBe(3n)
    expect(buy[2]).toBe(0n)
  })

  it('carries a sent payment on any later failure, so a retry resumes from it', async () => {
    const deps = makeDeps({
      fetchManaAllowance: vi.fn(async () => 10n ** 30n),
      publishLinkedItems: vi.fn(async () => {
        throw serverError(400, 'Bad request')
      })
    })

    const error = await failure(
      publishLinkedItems(makeParams({ thirdParty: { ...thirdParty, published: true } }), deps)
    )

    expect(error.step).toBe('publish')
    expect(error.txHash).toBe('0xtx1')
    expect(getResumablePayment(error)).toEqual({ txHash: '0xtx1', registration: false })
  })

  it('never offers a reverted payment to resume from', () => {
    const reverted = new LinkedPublishError('wait_tx', 'reverted', 'x', undefined, '0xtx1')
    reverted.payment = { txHash: '0xtx1', registration: false }
    expect(getResumablePayment(reverted)).toBeUndefined()
  })

  it('reports a purchase the moment it is sent and clears it once the publish succeeds', async () => {
    const onPayment = vi.fn()
    const deps = makeDeps({ fetchManaAllowance: vi.fn(async () => 10n ** 30n), onPayment })

    await publishLinkedItems(makeParams({ thirdParty: { ...thirdParty, published: true } }), deps)

    expect(onPayment.mock.calls).toEqual([[{ txHash: '0xtx1', registration: false }], [undefined]])
  })

  it('after a resumed payment, buys only the slots the registry still lacks for a bigger selection', async () => {
    const deps = makeDeps({
      fetchManaAllowance: vi.fn(async () => 10n ** 30n),
      fetchSlots: vi.fn(async () => ({ available: 2, registered: true }))
    })

    await publishLinkedItems(
      makeParams({ thirdParty: { ...thirdParty, published: true }, paidTx: { txHash: '0xold', registration: false } }),
      deps
    )

    expect(deps.settleTransaction).toHaveBeenCalledWith('0xold', undefined, expect.any(Function))
    expect(sentMethods(deps)).toEqual(['buyItemSlots'])
    expect(decodeSent(deps, 0)[1]).toBe(1n)
  })

  it('buys nothing when a resumed payment was sped up and its slots are on chain under another hash', async () => {
    const deps = makeDeps({
      settleTransaction: vi.fn(async () => 'dropped' as const),
      fetchSlots: vi.fn(async () => ({ available: 3, registered: true }))
    })

    await publishLinkedItems(
      makeParams({ thirdParty: { ...thirdParty, published: true }, paidTx: { txHash: '0xold', registration: false } }),
      deps
    )

    expect(deps.sendTransaction).not.toHaveBeenCalled()
    expect(deps.publishLinkedItems).toHaveBeenCalled()
  })

  it('forgets a resumed payment that reverted and pays for the slots again', async () => {
    const onPayment = vi.fn()
    const deps = makeDeps({
      fetchManaAllowance: vi.fn(async () => 10n ** 30n),
      settleTransaction: vi.fn(async (hash: string) => (hash === '0xold' ? ('reverted' as const) : ('mined' as const))),
      onPayment
    })

    await publishLinkedItems(
      makeParams({ thirdParty: { ...thirdParty, published: true }, paidTx: { txHash: '0xold', registration: false } }),
      deps
    )

    expect(onPayment.mock.calls[0]).toEqual([undefined])
    expect(sentMethods(deps)).toEqual(['buyItemSlots'])
  })

  it('stops without paying again while an earlier payment is still pending, and keeps it to resume later', async () => {
    const onPayment = vi.fn()
    const deps = makeDeps({ settleTransaction: vi.fn(async () => 'pending' as const), onPayment })

    const error = await failure(
      publishLinkedItems(
        makeParams({
          thirdParty: { ...thirdParty, published: true },
          paidTx: { txHash: '0xold', registration: false }
        }),
        deps
      )
    )

    expect(error.reason).toBe('payment_pending')
    expect(getResumablePayment(error)).toEqual({ txHash: '0xold', registration: false })
    expect(onPayment).not.toHaveBeenCalled()
    expect(deps.sendTransaction).not.toHaveBeenCalled()
  })

  it('goes on past a payment that never confirms once the registry holds its slots', async () => {
    const deps = makeDeps({
      settleTransaction: vi.fn(async () => 'pending' as const),
      fetchSlots: vi.fn(async () => ({ available: 3, registered: true }))
    })

    await publishLinkedItems(
      makeParams({ thirdParty: { ...thirdParty, published: true }, paidTx: { txHash: '0xold', registration: false } }),
      deps
    )

    expect(deps.sendTransaction).not.toHaveBeenCalled()
    expect(deps.publishLinkedItems).toHaveBeenCalled()
  })

  it('records the sender of a payment saved without one as soon as the watch sees it', async () => {
    const onPayment = vi.fn()
    const sender = { from: ADDRESS, nonce: 3 }
    const deps = makeDeps({
      settleTransaction: vi.fn(async (_hash: string, _sender, onSeen?: (seen: typeof sender) => void) => {
        onSeen?.(sender)
        return 'pending' as const
      }),
      onPayment
    })

    await failure(
      publishLinkedItems(
        makeParams({
          thirdParty: { ...thirdParty, published: true },
          paidTx: { txHash: '0xold', registration: false }
        }),
        deps
      )
    )

    expect(onPayment).toHaveBeenCalledWith({ txHash: '0xold', registration: false, sender })
  })

  it('forgets a never-confirming payment once the registry holds its slots', async () => {
    const onPayment = vi.fn()
    const deps = makeDeps({
      settleTransaction: vi.fn(async () => 'pending' as const),
      fetchSlots: vi.fn(async () => ({ available: 3, registered: true })),
      onPayment
    })

    await publishLinkedItems(
      makeParams({ thirdParty: { ...thirdParty, published: true }, paidTx: { txHash: '0xold', registration: false } }),
      deps
    )

    expect(onPayment).toHaveBeenCalledWith(undefined)
  })

  it('records who sent a purchase, and watches it by that sender and nonce', async () => {
    const onPayment = vi.fn()
    const sender = { from: ADDRESS, nonce: 9 }
    const deps = makeDeps({
      fetchManaAllowance: vi.fn(async () => 10n ** 30n),
      readSender: vi.fn(async () => sender),
      onPayment
    })

    await publishLinkedItems(makeParams({ thirdParty: { ...thirdParty, published: true } }), deps)

    expect(onPayment).toHaveBeenCalledWith({ txHash: '0xtx1', registration: false, sender })
    expect(deps.settleTransaction).toHaveBeenCalledWith('0xtx1', sender, expect.any(Function))
  })

  it('reports a purchase replaced by a cancel as charging nothing', async () => {
    const deps = makeDeps({
      fetchManaAllowance: vi.fn(async () => 10n ** 30n),
      settleTransaction: vi.fn(async () => 'dropped' as const),
      fetchSlots: vi.fn(async () => ({ available: 0, registered: true }))
    })

    const error = await failure(
      publishLinkedItems(makeParams({ thirdParty: { ...thirdParty, published: true } }), deps)
    )

    expect(error.reason).toBe('reverted')
    expect(deps.publishLinkedItems).not.toHaveBeenCalled()
  })

  it('refuses to pay more than the fee the creator confirmed', async () => {
    const deps = makeDeps({ fetchManaAllowance: vi.fn(async () => 10n ** 30n) })

    const error = await failure(
      publishLinkedItems(
        makeParams({
          thirdParty: { ...thirdParty, published: true },
          slots: { available: 0, registered: true },
          maxFeeManaWei: getLinkedPublishFee(1, price).maxManaWei
        }),
        deps
      )
    )

    expect(error.reason).toBe('fee_changed')
    expect(deps.sendTransaction).not.toHaveBeenCalled()
  })

  it('forgets the payment when a resumed publish fails because a review is pending', async () => {
    const onPayment = vi.fn()
    const deps = makeDeps({
      onPayment,
      publishLinkedItems: vi.fn(async () => {
        throw PENDING_REVIEW
      })
    })

    const error = await failure(
      publishLinkedItems(
        makeParams({
          thirdParty: { ...thirdParty, published: true },
          paidTx: { txHash: '0xpaid', registration: false }
        }),
        deps
      )
    )

    expect(error.reason).toBe('pending_review')
    expect(onPayment).toHaveBeenLastCalledWith(undefined)
  })

  it('resumes, then still reports insufficient slots without dropping the payment it waits on', async () => {
    const onPayment = vi.fn()
    const deps = makeDeps({
      onPayment,
      fetchSlots: vi.fn(async () => ({ available: 3, registered: true })),
      publishLinkedItems: vi.fn(async () => {
        throw INSUFFICIENT_SLOTS
      })
    })

    const error = await failure(
      publishLinkedItems(
        makeParams({
          thirdParty: { ...thirdParty, published: true },
          paidTx: { txHash: '0xpaid', registration: false }
        }),
        deps
      )
    )

    expect(error.reason).toBe('insufficient_slots')
    expect(getResumablePayment(error)).toEqual({ txHash: '0xpaid', registration: false })
    expect(onPayment).not.toHaveBeenCalledWith(undefined)
    expect(deps.sendTransaction).not.toHaveBeenCalled()
  })

  it('still removes the virtual third party when resuming a registration it now reads as published', async () => {
    const deps = makeDeps()
    await publishLinkedItems(
      makeParams({
        thirdParty: { ...thirdParty, published: true },
        paidTx: { txHash: '0xpaid', registration: true }
      }),
      deps
    )
    expect(deps.deleteVirtualThirdParty).toHaveBeenCalledWith(THIRD_PARTY_ID)
  })

  it('resumes after a payment an earlier attempt sent, without paying again', async () => {
    const deps = makeDeps({
      fetchManaAllowance: vi.fn(async () => 0n),
      fetchSlots: vi.fn(async () => ({ available: 200, registered: true }))
    })

    const result = await publishLinkedItems(
      makeParams({ paidTx: { txHash: '0xpaid', registration: true }, isProgrammatic: true }),
      deps
    )

    expect(deps.settleTransaction).toHaveBeenCalledWith('0xpaid', undefined, expect.any(Function))
    expect(deps.sendTransaction).not.toHaveBeenCalled()
    expect(deps.fetchManaAllowance).not.toHaveBeenCalled()
    expect(deps.setThirdPartyKind).not.toHaveBeenCalled()
    expect(deps.deleteVirtualThirdParty).toHaveBeenCalledWith(THIRD_PARTY_ID)
    expect(result.txHash).toBe('0xpaid')
    expect(steps(deps)).toEqual(['check', 'tos', 'wait_tx', 'delete_virtual', 'publish'])
  })

  it('neither pays nor publishes again when an earlier request already published the items, and still pushes', async () => {
    const onPayment = vi.fn()
    const deps = makeDeps({
      fetchCurations: vi.fn(
        async () =>
          new Map([
            ...itemsToPublish.map(item => [item.id, pendingCuration(item)] as const),
            ['p', { ...pendingCuration(itemsToPush[0]), status: CurationStatus.APPROVED, contentHash: 'old' }] as const
          ])
      ),
      onPayment
    })

    const result = await publishLinkedItems(
      makeParams({ itemsToPush, paidTx: { txHash: '0xpaid', registration: false } }),
      deps
    )

    expect(deps.sendTransaction).not.toHaveBeenCalled()
    expect(deps.publishLinkedItems).not.toHaveBeenCalled()
    expect(deps.pushChanges).toHaveBeenCalledWith(itemsToPush, expect.any(Map))
    // Never seen settling here: kept, so a later publish waits for it instead of paying again.
    expect(onPayment).not.toHaveBeenCalled()
    expect(result.itemCurations).toEqual([])
  })

  it('on a retry after a partial push failure, pushes only what is left, from curations read now', async () => {
    const [x, y] = [
      makeItem('x', { isPublished: true, isApproved: true, inCatalyst: true, catalystContentHash: 'old-x' }),
      makeItem('y', { isPublished: true, isApproved: true, inCatalyst: true, catalystContentHash: 'old-y' })
    ]
    const approved = (item: Item) => ({ ...pendingCuration(item), status: CurationStatus.APPROVED, contentHash: 'old' })
    const fresh = new Map<string, ItemCuration>([
      ...itemsToPublish.map(item => [item.id, pendingCuration(item)] as const),
      // X went through on the first attempt; Y did not.
      ['x', pendingCuration(x)],
      ['y', approved(y)]
    ])
    const deps = makeDeps({ fetchCurations: vi.fn(async () => fresh) })

    await publishLinkedItems(makeParams({ itemsToPush: [x, y] }), deps)

    expect(deps.publishLinkedItems).not.toHaveBeenCalled()
    expect(deps.pushChanges).toHaveBeenCalledWith([y], fresh)
  })

  it("counts a first attempt's pending-review answer as published when the items are pending", async () => {
    let reads = 0
    const deps = makeDeps({
      // Free before the publish; the earlier request whose answer was lost lands meanwhile.
      fetchCurations: vi.fn(async () =>
        ++reads === 1 ? new Map() : new Map(itemsToPublish.map(item => [item.id, pendingCuration(item)]))
      ),
      publishLinkedItems: vi.fn(async () => {
        throw PENDING_REVIEW
      })
    })

    const result = await publishLinkedItems(
      makeParams({ thirdParty: { ...thirdParty, published: true }, slots: { available: 3, registered: true } }),
      deps
    )

    expect(result.itemCurations).toEqual([])
  })

  it('waits for the registry before calling a replaced purchase a cancel', async () => {
    const deps = makeDeps({
      fetchManaAllowance: vi.fn(async () => 10n ** 30n),
      settleTransaction: vi.fn(async () => 'dropped' as const),
      fetchSlots: vi
        .fn()
        .mockResolvedValueOnce({ available: 0, registered: true })
        .mockResolvedValue({ available: 3, registered: true })
    })

    await publishLinkedItems(makeParams({ thirdParty: { ...thirdParty, published: true } }), deps)

    expect(deps.fetchSlots).toHaveBeenCalledTimes(2)
    expect(deps.publishLinkedItems).toHaveBeenCalled()
  })

  it('keeps an earlier payment it has not seen settle when another review is pending', async () => {
    const onPayment = vi.fn()
    const deps = makeDeps({
      fetchCurations: vi.fn(async () => new Map([['other', pendingCuration(makeItem('other'))]])),
      onPayment
    })

    const error = await failure(
      publishLinkedItems(makeParams({ paidTx: { txHash: '0xunmined', registration: false } }), deps)
    )

    expect(error.reason).toBe('pending_review')
    expect(onPayment).not.toHaveBeenCalled()
  })

  it('counts a slots answer after a server failure as published when the items are pending', async () => {
    let attempts = 0
    let reads = 0
    const deps = makeDeps({
      fetchCurations: vi.fn(async () =>
        ++reads === 1
          ? new Map()
          : new Map([
              ...itemsToPublish.map(item => [item.id, pendingCuration(item)] as const),
              [
                'p',
                { ...pendingCuration(itemsToPush[0]), status: CurationStatus.APPROVED, contentHash: 'old' }
              ] as const
            ])
      ),
      // The curations were written, then the forum post failed; the retry trips the slot check first.
      publishLinkedItems: vi.fn(async () => {
        throw ++attempts === 1 ? serverError(500, 'Discourse down') : INSUFFICIENT_SLOTS
      })
    })

    const result = await publishLinkedItems(
      makeParams({
        thirdParty: { ...thirdParty, published: true },
        slots: { available: 3, registered: true },
        itemsToPush
      }),
      deps
    )

    expect(deps.publishLinkedItems).toHaveBeenCalledTimes(2)
    expect(deps.pushChanges).toHaveBeenCalled()
    expect(result.itemCurations).toEqual([])
  })

  it('stops before paying while another review of the collection is pending', async () => {
    const deps = makeDeps({
      fetchCurations: vi.fn(async () => new Map([['other', { ...pendingCuration(makeItem('other')) }]]))
    })

    const error = await failure(publishLinkedItems(makeParams(), deps))

    expect(error.reason).toBe('pending_review')
    expect(deps.saveTOS).not.toHaveBeenCalled()
    expect(deps.sendTransaction).not.toHaveBeenCalled()
  })

  it('reports changes that could not be pushed after the items were published', async () => {
    const deps = makeDeps({
      pushChanges: vi.fn(async (items: Item[]) => ({
        pushed: [],
        failed: items.map(item => ({ item, message: 'boom' }))
      }))
    })

    const error = await failure(
      publishLinkedItems(
        makeParams({
          thirdParty: { ...thirdParty, published: true },
          slots: { available: 3, registered: true },
          itemsToPush
        }),
        deps
      )
    )

    expect(error.reason).toBe('push_failed')
    expect(deps.publishLinkedItems).toHaveBeenCalled()
  })

  it('does not retry a rejected ToS and reports it as tos_failed', async () => {
    const deps = makeDeps({
      saveTOS: vi.fn(async () => {
        throw serverError(400, 'Invalid email')
      })
    })

    const error = await failure(publishLinkedItems(makeParams(), deps))

    expect(error.step).toBe('tos')
    expect(error.reason).toBe('tos_failed')
    expect(deps.saveTOS).toHaveBeenCalledTimes(1)
    expect(deps.sendTransaction).not.toHaveBeenCalled()
  })

  it('retries a ToS that fails on the server side before giving up', async () => {
    const deps = makeDeps({
      saveTOS: vi.fn(async () => {
        throw serverError(503, 'Unavailable')
      })
    })

    const error = await failure(publishLinkedItems(makeParams(), deps))

    expect(error.reason).toBe('tos_failed')
    expect(deps.saveTOS).toHaveBeenCalledTimes(3)
  })

  it('refuses to publish items without a signed cheque', async () => {
    const deps = makeDeps()

    const error = await failure(publishLinkedItems(makeParams({ cheque: null }), deps))

    expect(error.step).toBe('publish')
    expect(error.reason).toBe('generic')
    expect(deps.saveTOS).not.toHaveBeenCalled()
  })
})

describe('toLinkedPublishError', () => {
  it('maps the known failures and keeps the step', () => {
    expect(toLinkedPublishError(INSUFFICIENT_SLOTS, 'publish')).toMatchObject({
      step: 'publish',
      reason: 'insufficient_slots'
    })
    expect(toLinkedPublishError(PENDING_REVIEW, 'publish').reason).toBe('pending_review')
    expect(toLinkedPublishError({ code: 'ACTION_REJECTED' }, 'buy_slots').reason).toBe('rejected')
    expect(toLinkedPublishError(new WrongAccountError('0xme', '0xother'), 'buy_slots').reason).toBe('wallet')
    expect(toLinkedPublishError(new Error('boom'), 'tos').reason).toBe('tos_failed')
    expect(toLinkedPublishError(new Error('boom'), 'push')).toMatchObject({
      step: 'push',
      reason: 'generic',
      message: 'boom'
    })
  })

  it('passes a LinkedPublishError through untouched', () => {
    const original = new LinkedPublishError('wait_tx', 'reverted', 'reverted', undefined, '0xtx')
    expect(toLinkedPublishError(original, 'publish')).toBe(original)
  })
})

describe('getLinkedSlots', () => {
  it('adds the slots the registry sold that builder-server has not indexed yet', () => {
    expect(getLinkedSlots(2, 5, { registered: true, maxItems: 8 })).toEqual({ available: 5, registered: true })
    expect(getLinkedSlots(2, 5, { registered: true, maxItems: 5 })).toEqual({ available: 2, registered: true })
    // A registration the server still reads as virtual.
    expect(getLinkedSlots(0, 0, { registered: true, maxItems: 3 })).toEqual({ available: 3, registered: true })
  })
})

describe('readFreshLinkedQuote', () => {
  const kind = { isProgrammatic: false, published: true }
  const confirmed = { slots: { available: 2, registered: true }, price }

  it('goes on with the fresh reads, capped at the fee the creator confirmed', async () => {
    const fresh = await readFreshLinkedQuote(3, kind, confirmed, {
      readSlots: async () => ({ available: 3, registered: true }),
      readPrice: async () => price
    })
    expect(fresh).toEqual({
      kind: 'ready',
      slots: { available: 3, registered: true },
      price,
      maxFeeManaWei: getLinkedPublishFee(1, price).maxManaWei
    })
  })

  it('asks again when fewer free slots or a higher price would cost more than confirmed', async () => {
    const fewer = await readFreshLinkedQuote(3, kind, confirmed, {
      readSlots: async () => ({ available: 1, registered: true }),
      readPrice: async () => price
    })
    expect(fewer.kind).toBe('fee_changed')
    const pricier = await readFreshLinkedQuote(3, kind, confirmed, {
      readSlots: async () => confirmed.slots,
      readPrice: async () => ({ ...price, itemSlotManaWei: price.itemSlotManaWei * 2n })
    })
    expect(pricier.kind).toBe('fee_changed')
  })

  it('confirms a read of zero free slots, and stops on a failed read', async () => {
    const readSlots = vi
      .fn()
      .mockResolvedValueOnce({ available: 0, registered: true })
      .mockResolvedValueOnce({ available: 2, registered: true })
    const fresh = await readFreshLinkedQuote(3, kind, confirmed, { readSlots, readPrice: async () => price })
    expect(readSlots).toHaveBeenCalledTimes(2)
    expect(fresh).toMatchObject({ kind: 'ready', slots: { available: 2 } })

    const failed = await readFreshLinkedQuote(3, kind, confirmed, {
      readSlots: async () => undefined,
      readPrice: async () => price
    })
    expect(failed.kind).toBe('read_failed')
  })
})

describe('linked publish phases', () => {
  it('shows paying only when the run buys slots or resumes a payment, and adds it if the run gets there anyway', () => {
    expect(getLinkedPublishPhases(true)).toEqual(['prepare', 'pay', 'submit'])
    expect(getLinkedPublishPhases(false)).toEqual(['prepare', 'submit'])
    expect(getLinkedPublishPhases(false, 'buy_slots')).toEqual(['prepare', 'pay', 'submit'])
  })

  it('groups every step of the run under one of them', () => {
    expect(['check', 'tos', 'kind'].map(step => getLinkedPublishPhase(step as LinkedPublishStep))).toEqual([
      'prepare',
      'prepare',
      'prepare'
    ])
    expect(getLinkedPublishPhase('wait_tx')).toBe('pay')
    expect(getLinkedPublishPhase('push')).toBe('submit')
  })
})
