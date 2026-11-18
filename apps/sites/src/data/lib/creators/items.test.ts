import { describe, it, expect } from 'vitest'
import { CollectionDisplayStatus } from './collections'
import {
  BODY_SHAPE_FEMALE,
  BODY_SHAPE_MALE,
  BodyShapeType,
  canDeleteItem,
  canEditItemDetails,
  canEditItemPrice,
  canManageItem,
  isMissingSmartWearableVideo,
  isSmartWearable,
  isSocialEmote,
  getItemBodyShapeType,
  getItemDisplayStatus,
  getItemMetadata,
  getItemSales,
  getMissingBodyShapeType,
  hasEmptyModelMetrics,
  ItemType,
  type Item
} from './items'
import { fromRemoteItem } from '@/data/lib/creators/builder/mappers'
import type { RemoteItem } from '@/data/lib/creators/builder/wire'

const MALE = 'urn:decentraland:off-chain:base-avatars:BaseMale'
const FEMALE = 'urn:decentraland:off-chain:base-avatars:BaseFemale'

const remote: RemoteItem = {
  id: 'i1',
  name: 'Pirate Hat',
  description: 'Yarr',
  thumbnail: 'thumbnail.png',
  eth_address: '0xabc',
  collection_id: 'c1',
  price: '1000',
  beneficiary: '0xbenef',
  rarity: 'legendary',
  is_published: true,
  is_approved: true,
  in_catalyst: true,
  type: ItemType.WEARABLE,
  data: {
    category: 'hat',
    representations: [{ bodyShapes: [MALE, FEMALE], mainFile: 'hat.glb', contents: ['hat.glb'] }]
  },
  contents: { 'thumbnail.png': 'Qmhash', 'hat.glb': 'Qmglb' },
  created_at: '2026-03-01T10:00:00Z',
  updated_at: '2026-04-02T10:00:00Z'
}

function makeItem(overrides: Partial<Item> = {}): Item {
  return { ...fromRemoteItem(remote), ...overrides }
}

describe('getItemBodyShapeType', () => {
  it('detects unisex, male-only, female-only and no representations', () => {
    expect(getItemBodyShapeType(makeItem())).toBe(BodyShapeType.BOTH)
    expect(
      getItemBodyShapeType(
        makeItem({ data: { representations: [{ bodyShapes: [MALE], mainFile: 'a', contents: [] }] } })
      )
    ).toBe(BodyShapeType.MALE)
    expect(
      getItemBodyShapeType(
        makeItem({ data: { representations: [{ bodyShapes: [FEMALE], mainFile: 'a', contents: [] }] } })
      )
    ).toBe(BodyShapeType.FEMALE)
    expect(getItemBodyShapeType(makeItem({ data: { representations: [] } }))).toBeNull()
  })
})

describe('getItemDisplayStatus', () => {
  it('derives published / under review / draft', () => {
    expect(getItemDisplayStatus(makeItem())).toBe(CollectionDisplayStatus.PUBLISHED)
    expect(getItemDisplayStatus(makeItem({ isApproved: false }))).toBe(CollectionDisplayStatus.UNDER_REVIEW)
    expect(getItemDisplayStatus(makeItem({ isPublished: false, isApproved: false }))).toBe(
      CollectionDisplayStatus.DRAFT
    )
  })
})

describe('getItemMetadata', () => {
  it('builds the legacy wearable metadata string', () => {
    expect(getItemMetadata(makeItem())).toBe('1:w:Pirate Hat:Yarr:hat:BaseMale,BaseFemale')
  })

  it('marks smart wearables by their .js contents', () => {
    const item = makeItem({ contents: { ...remote.contents, 'game.js': 'Qmjs' } })
    expect(getItemMetadata(item)).toBe('1:sw:Pirate Hat:Yarr:hat:BaseMale,BaseFemale')
  })

  it('appends loop, sound/props and outcome flags for emotes', () => {
    const emote = makeItem({
      type: ItemType.EMOTE,
      contents: { 'emote.glb': 'Qm', 'sound.mp3': 'Qms' },
      metrics: { props: 2 },
      data: {
        category: 'dance',
        loop: true,
        outcomes: [{}, {}],
        randomizeOutcomes: true,
        representations: [{ bodyShapes: [MALE, FEMALE], mainFile: 'emote.glb', contents: ['emote.glb'] }]
      }
    })
    expect(getItemMetadata(emote)).toBe('1:e:Pirate Hat:Yarr:dance:BaseMale,BaseFemale:1:sg:ro')
  })

  it('leaves plain emotes with just the loop flag', () => {
    const emote = makeItem({
      type: ItemType.EMOTE,
      contents: { 'emote.glb': 'Qm' },
      data: {
        category: 'dance',
        representations: [{ bodyShapes: [MALE], mainFile: 'emote.glb', contents: ['emote.glb'] }]
      }
    })
    expect(getItemMetadata(emote)).toBe('1:e:Pirate Hat:Yarr:dance:BaseMale:0')
  })

  it('throws when the item has no category', () => {
    expect(() => getItemMetadata(makeItem({ data: { representations: [] } }))).toThrow(/category/)
  })
})

describe('getMissingBodyShapeType', () => {
  const representation = (bodyShapes: string[]) => ({ bodyShapes, mainFile: 'f', contents: ['f'] })

  function itemWithShapes(bodyShapes: string[]): Item {
    return {
      id: 'i',
      name: 'n',
      description: '',
      thumbnail: 't',
      owner: '0x',
      isPublished: false,
      isApproved: false,
      inCatalyst: false,
      type: ItemType.WEARABLE,
      data: { representations: [representation(bodyShapes)] },
      contents: {},
      createdAt: 1,
      updatedAt: 1
    }
  }

  it('returns the opposite single shape, or null for unisex items', () => {
    expect(getMissingBodyShapeType(itemWithShapes([BODY_SHAPE_MALE]))).toBe(BodyShapeType.FEMALE)
    expect(getMissingBodyShapeType(itemWithShapes([BODY_SHAPE_FEMALE]))).toBe(BodyShapeType.MALE)
    expect(getMissingBodyShapeType(itemWithShapes([BODY_SHAPE_MALE, BODY_SHAPE_FEMALE]))).toBeNull()
  })
})

describe('getItemSales', () => {
  const base = fromRemoteItem(remote)

  it('pairs the minted count with the rarity max supply', () => {
    expect(getItemSales({ ...base, rarity: 'legendary', totalSupply: 15 })).toEqual({ minted: 15, maxSupply: 100 })
    expect(getItemSales({ ...base, rarity: 'legendary', totalSupply: undefined })).toEqual({
      minted: 0,
      maxSupply: 100
    })
  })

  it('has no sales without a rarity', () => {
    expect(getItemSales({ ...base, rarity: undefined })).toBeUndefined()
  })
})

describe('isSmartWearable', () => {
  it('is a wearable that ships scene code, never an emote', () => {
    expect(isSmartWearable(makeItem())).toBe(false)
    expect(isSmartWearable(makeItem({ contents: { ...remote.contents, 'game.js': 'Qmjs' } }))).toBe(true)
    expect(isSmartWearable(makeItem({ type: ItemType.EMOTE, contents: { 'game.js': 'Qmjs' } }))).toBe(false)
  })
})

describe('isSocialEmote', () => {
  it('flags emotes with extra armatures or a start animation, never a wearable', () => {
    const emote = makeItem({ type: ItemType.EMOTE })
    expect(isSocialEmote(emote)).toBe(false)
    expect(isSocialEmote({ ...emote, metrics: { additionalArmatures: 1 } })).toBe(true)
    expect(isSocialEmote({ ...emote, data: { ...emote.data, startAnimation: {} } })).toBe(true)
    expect(isSocialEmote(makeItem({ metrics: { additionalArmatures: 1 } }))).toBe(false)
  })
})

describe('isMissingSmartWearableVideo', () => {
  it('only flags smart wearables whose video.mp4 is not in the contents yet', () => {
    const smart = { ...remote.contents, 'male/bin/game.js': 'Qmjs' }
    expect(isMissingSmartWearableVideo(makeItem())).toBe(false)
    expect(isMissingSmartWearableVideo(makeItem({ contents: smart }))).toBe(true)
    expect(isMissingSmartWearableVideo(makeItem({ contents: { ...smart, 'video.mp4': 'Qmvideo' } }))).toBe(false)
  })
})

describe('canManageItem', () => {
  const collection = {
    id: 'c1',
    name: 'Hats',
    owner: '0xOwner',
    urn: 'urn',
    isPublished: false,
    isApproved: false,
    itemCount: 1,
    minters: ['0xMinter'],
    managers: ['0xManager'],
    createdAt: 1,
    updatedAt: 1
  }
  const item = fromRemoteItem({ ...remote, eth_address: '0xCreator' })

  it('lets the collection owner and its collaborators manage the item', () => {
    expect(canManageItem(collection, item, '0xowner')).toBe(true)
    expect(canManageItem(collection, item, '0xmanager')).toBe(true)
  })

  it('keeps minters, strangers and a creator with no role on the collection out', () => {
    expect(canManageItem(collection, item, '0xcreator')).toBe(false)
    expect(canManageItem(collection, item, '0xminter')).toBe(false)
    expect(canManageItem(collection, item, '0xother')).toBe(false)
    expect(canManageItem(collection, item, undefined)).toBe(false)
  })
})

describe('canDeleteItem', () => {
  const draft = {
    id: 'c1',
    name: 'Hats',
    owner: '0xOwner',
    urn: 'urn',
    isPublished: false,
    isApproved: false,
    itemCount: 1,
    minters: ['0xMinter'],
    managers: ['0xManager'],
    createdAt: 1,
    updatedAt: 1
  }

  it('lets only the collection owner delete an item from a draft', () => {
    expect(canDeleteItem(draft, '0xowner')).toBe(true)
    expect(canDeleteItem(draft, '0xmanager')).toBe(false)
    expect(canDeleteItem(draft, '0xminter')).toBe(false)
    expect(canDeleteItem(draft, undefined)).toBe(false)
  })

  it('refuses once the collection is published or under the publish lock', () => {
    expect(canDeleteItem({ ...draft, isPublished: true }, '0xowner')).toBe(false)
    expect(canDeleteItem({ ...draft, lock: Date.now() }, '0xowner')).toBe(false)
  })
})

describe('canEditItemDetails', () => {
  const collection = {
    id: 'c1',
    name: 'Hats',
    owner: '0xOwner',
    urn: 'urn',
    isPublished: false,
    isApproved: false,
    itemCount: 1,
    minters: ['0xMinter'],
    managers: ['0xManager'],
    createdAt: 1,
    updatedAt: 1
  }
  const item = fromRemoteItem({ ...remote, eth_address: '0xCreator' })

  it('lets whoever manages the item edit it, before and after publishing', () => {
    expect(canEditItemDetails(collection, item, '0xowner')).toBe(true)
    expect(canEditItemDetails({ ...collection, isPublished: true, isApproved: true }, item, '0xmanager')).toBe(true)
    expect(canEditItemDetails(collection, item, '0xminter')).toBe(false)
    expect(canEditItemDetails(collection, item, undefined)).toBe(false)
  })

  it('blocks edits while the collection is under the publish lock', () => {
    expect(canEditItemDetails({ ...collection, lock: Date.now() }, item, '0xowner')).toBe(false)
  })
})

describe('canEditItemPrice', () => {
  const approved = {
    id: 'c1',
    name: 'Hats',
    owner: '0xOwner',
    urn: 'urn',
    isPublished: true,
    isApproved: true,
    itemCount: 1,
    minters: ['0xMinter'],
    managers: ['0xManager'],
    createdAt: 1,
    updatedAt: 1
  }
  const item = fromRemoteItem({ ...remote, total_supply: 10 })
  const order = { itemId: '3', tradeId: 'trade-1', currency: 'credits' as const, credits: 5 }

  it('lets only the owner re-price an off-chain order', () => {
    expect(canEditItemPrice(approved, item, order, '0xowner')).toBe(true)
    expect(canEditItemPrice(approved, item, order, '0xmanager')).toBe(false)
    expect(canEditItemPrice(approved, item, order, '0xminter')).toBe(false)
    expect(canEditItemPrice(approved, item, order, '0xother')).toBe(false)
    expect(canEditItemPrice(approved, item, order, undefined)).toBe(false)
  })

  it('refuses legacy store prices, unlisted items, sold-out items and unapproved collections', () => {
    expect(canEditItemPrice(approved, item, { itemId: '3', currency: 'mana', manaWei: 1n }, '0xowner')).toBe(false)
    expect(canEditItemPrice(approved, item, null, '0xowner')).toBe(false)
    expect(canEditItemPrice(approved, item, undefined, '0xowner')).toBe(false)
    expect(canEditItemPrice(approved, { ...item, totalSupply: 100 }, order, '0xowner')).toBe(false)
    expect(canEditItemPrice({ ...approved, isApproved: false }, item, order, '0xowner')).toBe(false)
  })
})

describe('hasEmptyModelMetrics', () => {
  it('flags the empty boot scene reading, which still counts one mesh', () => {
    expect(hasEmptyModelMetrics({ triangles: 0, materials: 0, textures: 0, meshes: 1, bodies: 1, entities: 1 })).toBe(
      true
    )
  })

  it('flags metrics with no geometry fields at all', () => {
    expect(hasEmptyModelMetrics({})).toBe(true)
  })

  it('accepts a model with geometry', () => {
    expect(hasEmptyModelMetrics({ triangles: 540, materials: 1, textures: 0, meshes: 1 })).toBe(false)
    expect(hasEmptyModelMetrics({ triangles: 0, materials: 1, textures: 0 })).toBe(false)
  })
})
