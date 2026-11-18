import { describe, expect, it } from 'vitest'
import { toTransaction } from '~/test/chain'
import { ContractName, getContract } from '~/lib/chain/contracts'
import { type Collection } from './collections'
import {
  buildSetRolesCall,
  diffRoles,
  getRoleAddressError,
  getRoleAddresses,
  isCollectionOwner,
  withRoles
} from './collectionRoles'

const OWNER = '0x00000000000000000000000000000000000000aa'
const MINTER = '0x00000000000000000000000000000000000000bb'
const MANAGER = '0x00000000000000000000000000000000000000cc'
const OTHER = '0x00000000000000000000000000000000000000dd'
const CONTRACT = '0x00000000000000000000000000000000000000ee'
const CHAIN_ID = 80002
const MARKETPLACE = getContract(ContractName.OffChainMarketplaceV2, CHAIN_ID).address
const STORE = getContract(ContractName.CollectionStore, CHAIN_ID).address
const MARKETPLACE_V3 = getContract(ContractName.OffChainMarketplaceV3, CHAIN_ID).address

const collection: Collection = {
  id: 'c1',
  name: 'Hats',
  owner: OWNER,
  urn: `urn:decentraland:amoy:collections-v2:${CONTRACT}`,
  contractAddress: CONTRACT,
  isPublished: true,
  isApproved: true,
  itemCount: 2,
  minters: [MINTER.toUpperCase(), MARKETPLACE, STORE.toUpperCase(), MINTER, MARKETPLACE_V3],
  managers: [MANAGER],
  createdAt: 1,
  updatedAt: 1
}

describe('role lists', () => {
  it('lists senders without the sale contracts, lowercased and without repeats', () => {
    expect(getRoleAddresses(collection, 'senders', CHAIN_ID)).toEqual([MINTER])
    expect(getRoleAddresses(collection, 'collaborators', CHAIN_ID)).toEqual([MANAGER])
  })

  it('recognises the owner regardless of casing', () => {
    expect(isCollectionOwner(collection, OWNER.toUpperCase())).toBe(true)
    expect(isCollectionOwner(collection, OTHER)).toBe(false)
    expect(isCollectionOwner(collection, undefined)).toBe(false)
  })

  it('refuses an address already listed and the sale contracts, but lets the owner list themselves', () => {
    const current = [MINTER]
    expect(getRoleAddressError('senders', current, MINTER.toUpperCase(), CHAIN_ID)).toBe('duplicate')
    expect(getRoleAddressError('senders', current, MARKETPLACE, CHAIN_ID)).toBe('system')
    expect(getRoleAddressError('collaborators', current, MARKETPLACE, CHAIN_ID)).toBeNull()
    expect(getRoleAddressError('senders', current, OTHER, CHAIN_ID)).toBeNull()
    expect(getRoleAddressError('senders', current, OWNER, CHAIN_ID)).toBeNull()
  })
})

describe('saving roles', () => {
  it('sends only the changes: removals as false, additions as true', () => {
    expect(diffRoles([MINTER, MANAGER], [MANAGER, OTHER])).toEqual({
      addresses: [MINTER, OTHER],
      values: [false, true]
    })
    expect(diffRoles([MINTER], [MINTER]).addresses).toEqual([])
  })

  it('encodes setMinters / setManagers on the collection contract', () => {
    const diff = diffRoles([MINTER], [OTHER])
    const senders = buildSetRolesCall(collection, 'senders', diff, CHAIN_ID)
    expect(senders.contract.address).toBe(CONTRACT)
    expect(senders.method).toBe('setMinters')
    expect(senders.args).toEqual([
      [MINTER, OTHER],
      [false, true]
    ])
    expect(toTransaction(senders)).toEqual({
      to: '0x00000000000000000000000000000000000000ee',
      data:
        '0x41bceced' +
        '0000000000000000000000000000000000000000000000000000000000000040' +
        '00000000000000000000000000000000000000000000000000000000000000a0' +
        '0000000000000000000000000000000000000000000000000000000000000002' +
        '00000000000000000000000000000000000000000000000000000000000000bb' +
        '00000000000000000000000000000000000000000000000000000000000000dd' +
        '0000000000000000000000000000000000000000000000000000000000000002' +
        '0000000000000000000000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000001'
    })

    const collaborators = buildSetRolesCall(collection, 'collaborators', diff, CHAIN_ID)
    expect(collaborators.method).toBe('setManagers')
    expect(toTransaction(collaborators)).toEqual({
      to: '0x00000000000000000000000000000000000000ee',
      data:
        '0x3bdcc923' +
        '0000000000000000000000000000000000000000000000000000000000000040' +
        '00000000000000000000000000000000000000000000000000000000000000a0' +
        '0000000000000000000000000000000000000000000000000000000000000002' +
        '00000000000000000000000000000000000000000000000000000000000000bb' +
        '00000000000000000000000000000000000000000000000000000000000000dd' +
        '0000000000000000000000000000000000000000000000000000000000000002' +
        '0000000000000000000000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000001'
    })
  })

  it('refuses a collection without a contract', () => {
    expect(() =>
      buildSetRolesCall({ ...collection, contractAddress: undefined }, 'senders', diffRoles([], [OTHER]), CHAIN_ID)
    ).toThrow(/no contract/)
  })

  it('patches the collection keeping the sale contracts among the minters', () => {
    const patched = withRoles(collection, 'senders', [OTHER], CHAIN_ID)
    expect(patched.minters).toEqual([MARKETPLACE, STORE.toUpperCase(), MARKETPLACE_V3, OTHER])
    expect(withRoles(collection, 'collaborators', [], CHAIN_ID).managers).toEqual([])
  })
})
