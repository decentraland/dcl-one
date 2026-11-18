import { afterEach, describe, expect, it, vi } from 'vitest'
import { toTransaction } from '~/test/chain'
import { ContractName, getContract } from '~/lib/chain/contracts'
import { BuilderServerError } from './builder'
import { type Collection } from './collections'
import {
  buildTransferOwnershipCall,
  getNewOwnerError,
  matchesCollectionName,
  waitForOwnerSync
} from './collectionOwnership'

const OWNER = '0x00000000000000000000000000000000000000aa'
const OTHER = '0x00000000000000000000000000000000000000dd'
const CONTRACT = '0x00000000000000000000000000000000000000ee'
const CHAIN_ID = 80002

const collection: Collection = {
  id: 'c1',
  name: 'Pirate Hats',
  owner: OWNER,
  urn: `urn:decentraland:amoy:collections-v2:${CONTRACT}`,
  contractAddress: CONTRACT,
  isPublished: true,
  isApproved: true,
  itemCount: 2,
  minters: [],
  managers: [],
  createdAt: 1,
  updatedAt: 1
}

describe('choosing the new owner', () => {
  it("refuses the owner's own address, the zero address, the Shop's contracts and the collection's own contract, and accepts anyone else", () => {
    expect(getNewOwnerError(collection, OWNER.toUpperCase(), CHAIN_ID)).toBe('self')
    expect(getNewOwnerError(collection, '0x0000000000000000000000000000000000000000', CHAIN_ID)).toBe('zero')
    const marketplace = getContract(ContractName.OffChainMarketplaceV2, CHAIN_ID).address
    expect(getNewOwnerError(collection, marketplace.toUpperCase(), CHAIN_ID)).toBe('system')
    expect(getNewOwnerError(collection, CONTRACT.toUpperCase(), CHAIN_ID)).toBe('contract')
    expect(getNewOwnerError(collection, OTHER, CHAIN_ID)).toBeNull()
  })

  it('matches the typed confirmation to the collection name, ignoring case and surrounding spaces', () => {
    expect(matchesCollectionName(collection, '  pirate hats ')).toBe(true)
    expect(matchesCollectionName(collection, 'Pirate Hat')).toBe(false)
    expect(matchesCollectionName(collection, '')).toBe(false)
  })
})

describe('transferring', () => {
  it('calls transferCreatorship on the collection contract', () => {
    const call = buildTransferOwnershipCall(collection, OTHER, CHAIN_ID)
    expect(toTransaction(call)).toEqual({
      to: '0x00000000000000000000000000000000000000ee',
      data: '0x6d2e4b1b00000000000000000000000000000000000000000000000000000000000000dd'
    })
  })

  it('refuses a collection without a contract', () => {
    expect(() => buildTransferOwnershipCall({ ...collection, contractAddress: undefined }, OTHER, CHAIN_ID)).toThrow(
      /not_published|contract/
    )
  })
})

describe('waiting for the server to store the new owner', () => {
  afterEach(() => vi.useRealTimers())

  it('resolves with the collection once the server reports the new owner', async () => {
    const sync = vi
      .fn()
      .mockResolvedValueOnce(collection)
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValueOnce({ ...collection, owner: OTHER.toUpperCase() })
    await expect(waitForOwnerSync(collection.id, OTHER, { syncCollectionOwner: sync }, 5, 0)).resolves.toMatchObject({
      owner: OTHER.toUpperCase()
    })
    expect(sync).toHaveBeenCalledTimes(3)
  })

  it('gives up after ten minutes', async () => {
    vi.useFakeTimers()
    const sync = vi.fn().mockResolvedValue(collection)
    const run = waitForOwnerSync(collection.id, OTHER, { syncCollectionOwner: sync })
    const failure = expect(run).rejects.toThrow(/indexed/)
    await vi.advanceTimersByTimeAsync(10 * 60 * 1000)
    await failure
    expect(sync).toHaveBeenCalledTimes(121)
  })

  it('fails straight away when the server refuses the request', async () => {
    const sync = vi.fn().mockRejectedValue(new BuilderServerError('no such route', 404))
    await expect(waitForOwnerSync(collection.id, OTHER, { syncCollectionOwner: sync }, 5, 0)).rejects.toThrow(
      'no such route'
    )
    expect(sync).toHaveBeenCalledTimes(1)
  })
})
