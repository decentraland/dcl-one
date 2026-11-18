import { describe, it, expect } from 'vitest'
import { decodeAbiParameters, parseAbiParameters, slice, zeroAddress, type Hex } from 'viem'
import { config } from '~/config'
import { ItemType, type Item } from './items'
import {
  buildCollectionInitializeData,
  buildDefaultCatalystCollectionURN,
  buildNewCollection,
  getCollectionSymbol,
  toInitializeItems
} from './saveCollection'

// Lowercase like every address that leaves lib/auth (mixed case would fail the checksum).
const OWNER = '0xabc0000000000000000000000000000000000001'

function makeItem(overrides: Partial<Item> = {}): Item {
  return {
    id: 'i1',
    name: 'Pirate Hat',
    description: 'Yarr',
    thumbnail: 'thumbnail.png',
    owner: OWNER,
    rarity: 'Legendary',
    isPublished: false,
    isApproved: false,
    inCatalyst: false,
    type: ItemType.WEARABLE,
    data: {
      category: 'hat',
      representations: [
        { bodyShapes: ['urn:decentraland:off-chain:base-avatars:BaseMale'], mainFile: 'hat.glb', contents: [] }
      ]
    },
    contents: {},
    createdAt: 1000,
    updatedAt: 1000,
    ...overrides
  }
}

describe('getCollectionSymbol', () => {
  it('strips lowercase vowels and whitespace, uppercases, and prefixes DCL-', () => {
    // Exact legacy behavior: only lowercase vowels are stripped.
    expect(getCollectionSymbol('Pirate Hats')).toBe('DCL-PRTHTS')
    expect(getCollectionSymbol('Halloween')).toBe('DCL-HLLWN')
    expect(getCollectionSymbol('AEIOU aeiou')).toBe('DCL-AEIOU')
  })
})

describe('buildDefaultCatalystCollectionURN', () => {
  it('uses the env URN protocol and the zero address', () => {
    expect(buildDefaultCatalystCollectionURN()).toBe(
      `urn:decentraland:${config.get('MATIC_URN_PROTOCOL')}:collections-v2:0x0000000000000000000000000000000000000000`
    )
  })
})

describe('buildNewCollection', () => {
  it('creates an unpublished draft owned by the given address', () => {
    const collection = buildNewCollection('Pirate Hats', OWNER)
    expect(collection).toMatchObject({
      name: 'Pirate Hats',
      owner: OWNER,
      isPublished: false,
      isApproved: false,
      itemCount: 0,
      minters: [],
      managers: []
    })
    expect(collection.id).toMatch(/^[0-9a-f-]{36}$/)
    expect(collection.urn).toBe(buildDefaultCatalystCollectionURN())
    expect(collection.createdAt).toBe(collection.updatedAt)
  })

  it('gives every collection its own id', () => {
    expect(buildNewCollection('A', OWNER).id).not.toBe(buildNewCollection('A', OWNER).id)
  })
})

describe('toInitializeItems', () => {
  it('encodes items sorted by creation date with legacy defaults', () => {
    const items = [
      makeItem({ id: 'newer', createdAt: 2000, price: '5', beneficiary: '0x0000000000000000000000000000000000000002' }),
      makeItem({ id: 'older', createdAt: 1000 })
    ]
    expect(toInitializeItems(items)).toEqual([
      ['legendary', '0', zeroAddress, '1:w:Pirate Hat:Yarr:hat:BaseMale'],
      ['legendary', '5', '0x0000000000000000000000000000000000000002', '1:w:Pirate Hat:Yarr:hat:BaseMale']
    ])
  })
})

describe('buildCollectionInitializeData', () => {
  it('encodes calldata that decodes back to the initialize arguments', () => {
    const collection = buildNewCollection('Pirate Hats', OWNER)
    const data = buildCollectionInitializeData(collection, [makeItem()], OWNER)

    const decoded = decodeAbiParameters(
      parseAbiParameters('string, string, string, address, bool, bool, address, (string,uint256,address,string)[]'),
      slice(data as Hex, 4)
    )
    expect(decoded[0]).toBe('Pirate Hats')
    expect(decoded[1]).toBe('DCL-PRTHTS')
    expect(decoded[2]).toBe(config.get('ERC721_COLLECTION_BASE_URI'))
    expect(decoded[3].toLowerCase()).toBe(OWNER.toLowerCase())
    expect(decoded[4]).toBe(true) // should complete
    expect(decoded[5]).toBe(false) // is approved
    expect(decoded[6].toLowerCase()).toBe(config.get('RARITIES_WITH_ORACLE_ADDRESS').toLowerCase())
    expect(decoded[7]).toHaveLength(1)
    expect(decoded[7][0][0]).toBe('legendary')
    expect(decoded[7][0][3]).toBe('1:w:Pirate Hat:Yarr:hat:BaseMale')
  })

  it('encodes the exact calldata builder-server derives the contract address from', () => {
    const collection = buildNewCollection('Pirate Hats', OWNER)
    expect(buildCollectionInitializeData(collection, [makeItem()], OWNER)).toBe(
      '0xd14fee77' +
        '0000000000000000000000000000000000000000000000000000000000000100' +
        '0000000000000000000000000000000000000000000000000000000000000140' +
        '0000000000000000000000000000000000000000000000000000000000000180' +
        '000000000000000000000000abc0000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000000' +
        '00000000000000000000000025b6b4bac4adb582a0abd475439da6730777fbf7' +
        '0000000000000000000000000000000000000000000000000000000000000200' +
        '000000000000000000000000000000000000000000000000000000000000000b' +
        '5069726174652048617473000000000000000000000000000000000000000000' +
        '000000000000000000000000000000000000000000000000000000000000000a' +
        '44434c2d50525448545300000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000043' +
        '68747470733a2f2f706565722e646563656e7472616c616e642e7a6f6e652f6c' +
        '616d626461732f636f6c6c656374696f6e732f7374616e646172642f65726337' +
        '32312f0000000000000000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000020' +
        '0000000000000000000000000000000000000000000000000000000000000080' +
        '0000000000000000000000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000000' +
        '00000000000000000000000000000000000000000000000000000000000000c0' +
        '0000000000000000000000000000000000000000000000000000000000000009' +
        '6c6567656e646172790000000000000000000000000000000000000000000000' +
        '0000000000000000000000000000000000000000000000000000000000000020' +
        '313a773a506972617465204861743a596172723a6861743a426173654d616c65'
    )
  })
})
