// @vitest-environment jsdom
import './test/setup'
import { describe, it, expect } from 'vitest'
import type { ContractNetwork } from '@dcl/schemas'
import {
  buildLinkedItemUrn,
  buildNewLinkedCollection,
  canViewLinkedCollection,
  decodeLinkedUrn,
  getDefaultUrnSuffix,
  getLinkedContractNetworks,
  getThirdPartyId,
  isThirdPartyManager,
  isValidUrnSuffix,
  slugify,
  toRawMetadata,
  validateLinkedCollectionName,
  withCollectionUrnSuffix,
  withContractReplaced
} from './thirdParty'

const CONTRACT = { network: 'amoy' as ContractNetwork, address: '0x1D9FB685C257E74F869BA302E260C0B68F5EBB37' }

describe('slugify / getDefaultUrnSuffix', () => {
  it('lowercases, strips accents and joins words with dashes', () => {
    expect(slugify('Pirate Hats – Édition 2')).toBe('pirate-hats-edition-2')
  })

  it('suffixes the slug with four hex characters, falling back to "default" for an empty name', () => {
    expect(getDefaultUrnSuffix('Pirate Hats')).toMatch(/^pirate-hats-[0-9a-f]{4}$/)
    expect(getDefaultUrnSuffix('   ')).toMatch(/^default-[0-9a-f]{4}$/)
  })
})

describe('decodeLinkedUrn', () => {
  it('splits a collection URN into its third party and collection suffix', () => {
    expect(decodeLinkedUrn('urn:decentraland:amoy:collections-thirdparty:brand:hats')).toEqual({
      thirdPartyId: 'urn:decentraland:amoy:collections-thirdparty:brand',
      collectionSuffix: 'hats'
    })
  })

  it('also reads the item suffix of an item URN', () => {
    expect(decodeLinkedUrn('urn:decentraland:amoy:collections-thirdparty:brand:hats:cap-1')?.itemSuffix).toBe('cap-1')
  })

  it('is null for standard URNs and malformed ones', () => {
    expect(decodeLinkedUrn('urn:decentraland:amoy:collections-v2:0xcontract')).toBeNull()
    expect(decodeLinkedUrn('urn:decentraland:amoy:collections-thirdparty:brand')).toBeNull()
    expect(decodeLinkedUrn('urn:decentraland:amoy:collections-thirdparty:brand::x')).toBeNull()
    expect(decodeLinkedUrn(undefined)).toBeNull()
  })
})

describe('getThirdPartyId', () => {
  it('is the collection URN without its collection suffix', () => {
    expect(getThirdPartyId({ urn: 'urn:decentraland:amoy:collections-thirdparty:brand:hats' })).toBe(
      'urn:decentraland:amoy:collections-thirdparty:brand'
    )
  })

  it('is absent for standard collections', () => {
    expect(getThirdPartyId({ urn: 'urn:decentraland:amoy:collections-v2:0xcontract' })).toBeUndefined()
  })
})

describe('isThirdPartyManager', () => {
  const thirdParty = { managers: ['0xAbC0000000000000000000000000000000000001'] }

  it("lets the third party's managers in, whatever the address casing", () => {
    expect(isThirdPartyManager(thirdParty, '0xabc0000000000000000000000000000000000001')).toBe(true)
  })

  it('keeps everyone else out', () => {
    expect(isThirdPartyManager(thirdParty, '0xdef0000000000000000000000000000000000002')).toBe(false)
    expect(isThirdPartyManager(thirdParty, undefined)).toBe(false)
  })
})

describe('canViewLinkedCollection', () => {
  const thirdParty = { managers: ['0xabc0000000000000000000000000000000000001'] }
  const manager = '0xabc0000000000000000000000000000000000001'
  const stranger = '0xdef0000000000000000000000000000000000002'

  it('opens a published collection to any signer', () => {
    expect(canViewLinkedCollection({ isPublished: true }, thirdParty, manager)).toBe(true)
    expect(canViewLinkedCollection({ isPublished: true }, thirdParty, stranger)).toBe(true)
    expect(canViewLinkedCollection({ isPublished: true }, thirdParty, undefined)).toBe(false)
  })

  it('keeps an unpublished collection to its managers', () => {
    expect(canViewLinkedCollection({ isPublished: false }, thirdParty, manager)).toBe(true)
    expect(canViewLinkedCollection({ isPublished: false }, thirdParty, stranger)).toBe(false)
  })
})

describe('buildNewLinkedCollection', () => {
  it('gives the collection its own third party and links the contract, address lowercased', () => {
    const collection = buildNewLinkedCollection('Pirate Hats', '0xowner', CONTRACT, 'pirate-hats-ab12')
    expect(collection.urn).toBe(`urn:decentraland:amoy:collections-thirdparty:pirate-hats-ab12:${collection.id}`)
    expect(collection.linkedContractAddress).toBe(CONTRACT.address.toLowerCase())
    expect(collection.linkedContractNetwork).toBe('amoy')
    expect(collection).toMatchObject({ owner: '0xowner', isPublished: false, minters: [], managers: [] })
  })
})

describe('withCollectionUrnSuffix / buildLinkedItemUrn', () => {
  const collection = buildNewLinkedCollection('Hats', '0xowner', CONTRACT, 'brand')

  it('replaces only the collection suffix', () => {
    expect(withCollectionUrnSuffix(collection, 'summer-hats').urn).toBe(
      'urn:decentraland:amoy:collections-thirdparty:brand:summer-hats'
    )
  })

  it('appends the item suffix to the collection URN', () => {
    expect(buildLinkedItemUrn(collection.urn, 'cap-01')).toBe(`${collection.urn}:cap-01`)
  })

  it('refuses a standard collection', () => {
    expect(() =>
      withCollectionUrnSuffix({ ...collection, urn: 'urn:decentraland:amoy:collections-v2:0x1' }, 'x')
    ).toThrow()
  })
})

describe('isValidUrnSuffix', () => {
  it('accepts anything without colons or whitespace', () => {
    expect(isValidUrnSuffix('summer-hats_2')).toBe(true)
    expect(isValidUrnSuffix('summer hats')).toBe(false)
    expect(isValidUrnSuffix('a:b')).toBe(false)
    expect(isValidUrnSuffix('')).toBe(false)
  })
})

describe('validateLinkedCollectionName', () => {
  it('allows up to 42 characters without colons', () => {
    expect(validateLinkedCollectionName('x'.repeat(42))).toBeNull()
    expect(validateLinkedCollectionName('x'.repeat(43))).toBe('too_long')
    expect(validateLinkedCollectionName('a:b')).toBe('invalid_character')
    expect(validateLinkedCollectionName('  ')).toBe('empty')
  })
})

describe('withContractReplaced', () => {
  const sibling = { network: 'matic' as ContractNetwork, address: '0x' + '1'.repeat(40) }
  const to = { network: 'amoy' as ContractNetwork, address: '0x' + '2'.repeat(40) }

  it('drops every replaced contract, whatever its casing, and lists the new one once', () => {
    const listed = [sibling, { ...CONTRACT, address: CONTRACT.address.toLowerCase() }]
    expect(withContractReplaced(listed, [CONTRACT, null], to)).toEqual([sibling, to])
    expect(withContractReplaced([sibling, to], [CONTRACT], to)).toEqual([sibling, to])
  })

  it('keeps the new contract when it is also among the replaced ones', () => {
    expect(withContractReplaced([to], [to], to)).toEqual([to])
  })
})

describe('toRawMetadata', () => {
  it('encodes the third party as the registry stores it', () => {
    expect(toRawMetadata({ name: 'Brand', description: '', contracts: [CONTRACT] })).toBe(
      `tp:1:Brand::amoy-${CONTRACT.address}`
    )
    expect(toRawMetadata({ name: 'Brand', description: 'desc', contracts: [] })).toBe('tp:1:Brand:desc')
  })
})

describe('getLinkedContractNetworks', () => {
  it('hides the testnets in production', () => {
    expect(getLinkedContractNetworks(false)).toEqual(['mainnet', 'matic', 'sepolia', 'amoy'])
    expect(getLinkedContractNetworks(true)).toEqual(['mainnet', 'matic'])
  })
})
