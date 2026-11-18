import { beforeEach, describe, expect, it, vi } from 'vitest'
import { manifest } from '@dcl-regenesislabs/wearable-validator'
import { buildItem, withReplacedRepresentation, type ItemDraftPayload } from './itemFactory'
import { ItemFileError } from './itemFiles'
import { BODY_SHAPE_FEMALE, BODY_SHAPE_MALE, BodyShapeType, ItemType, type Item } from './items'
import {
  assertItemSize,
  changesItemSize,
  formatItemSize,
  getItemSizeLimit,
  guardItemSize,
  isOverSizeLimit,
  measureItem,
  sumUniqueContent
} from './itemSize'

vi.mock('./monitoring', () => ({ captureError: vi.fn() }))
const { captureError } = await import('./monitoring')

const MB = 1024 * 1024
const bytes = (size: number) => new Blob([new Uint8Array(size)])

// "Bull&Bear Merch" (Builder item 50b42c0b-57fa-4d41-adc0-ede21ae1cbb8): saved one body shape at a time, measured
// ~1.5 MB per save by the legacy Builder, 3,243,672 bytes in all — over the 3 MB the catalyst allows.
const BULL_AND_BEAR = {
  thumbnail: 201_089,
  image: 87_467,
  female: 1_537_136,
  male: 1_417_980
}

function storedItem(overrides: Partial<Item> = {}): Item {
  return {
    id: 'bull-bear',
    name: 'Bull&Bear Merch',
    description: '',
    thumbnail: 'thumbnail.png',
    owner: '0xowner',
    collectionId: 'col-1',
    rarity: 'epic',
    isPublished: false,
    isApproved: false,
    inCatalyst: false,
    type: ItemType.WEARABLE,
    data: {
      category: 'upper_body',
      representations: [
        { bodyShapes: [BODY_SHAPE_MALE], mainFile: 'male/shirt.glb', contents: ['male/shirt.glb'] },
        { bodyShapes: [BODY_SHAPE_FEMALE], mainFile: 'female/shirt.glb', contents: ['female/shirt.glb'] }
      ]
    },
    contents: {
      'male/shirt.glb': 'bafmale',
      'female/shirt.glb': 'baffemale',
      'thumbnail.png': 'bafthumb',
      'image.png': 'bafimage'
    },
    createdAt: 1,
    updatedAt: 1,
    ...overrides
  }
}

const STORED: Record<string, number> = {
  bafmale: BULL_AND_BEAR.male,
  baffemale: BULL_AND_BEAR.female,
  bafthumb: BULL_AND_BEAR.thumbnail,
  bafimage: BULL_AND_BEAR.image,
  bafvideo: 50 * MB
}
const sizeOf = vi.fn(async (hash: string) => {
  if (!(hash in STORED)) throw new Error(`unknown ${hash}`)
  return STORED[hash]
})

beforeEach(() => {
  sizeOf.mockClear()
  vi.mocked(captureError).mockReset()
})

describe('getItemSizeLimit', () => {
  it('allows 3 MB for wearables and emotes and 9 MB for skins, the validator manifest limits', () => {
    expect(getItemSizeLimit(ItemType.WEARABLE, 'hat')).toBe(manifest.fileSize.wearableBytes)
    expect(getItemSizeLimit(ItemType.WEARABLE, 'skin')).toBe(manifest.fileSize.skinBytes)
    expect(getItemSizeLimit(ItemType.EMOTE, 'dance')).toBe(manifest.fileSize.emoteBytes)
    expect(getItemSizeLimit(ItemType.WEARABLE, 'skin')).toBe(9 * MB)
  })
})

describe('sumUniqueContent', () => {
  it('counts a content hash once, however many paths point at it', () => {
    expect(
      sumUniqueContent([
        { hash: 'a', bytes: 10 },
        { hash: 'a', bytes: 10 },
        { hash: 'b', bytes: 5 }
      ])
    ).toBe(15)
  })
})

describe('measureItem', () => {
  it('counts every file the item deploys, thumbnail and rarity image included', async () => {
    const size = await measureItem({ item: storedItem() }, { sizeOf })
    expect(size).toEqual({ bytes: 3_243_672, limit: 3 * MB })
    expect(isOverSizeLimit(size)).toBe(true)
    expect(formatItemSize(size)).toEqual({ size: '3.09', limit: '3.00' })
  })

  it('counts a file both body shapes share once', async () => {
    const shared = storedItem({
      contents: {
        'male/shirt.glb': 'bafmale',
        'female/shirt.glb': 'bafmale',
        'thumbnail.png': 'bafthumb',
        'image.png': 'bafimage'
      }
    })
    const { bytes: total } = await measureItem({ item: shared }, { sizeOf })
    expect(total).toBe(BULL_AND_BEAR.male + BULL_AND_BEAR.thumbnail + BULL_AND_BEAR.image)
    expect(sizeOf.mock.calls.filter(([hash]) => hash === 'bafmale')).toHaveLength(1)
  })

  it('counts one shared blob once in a freshly built unisex item, with its thumbnail and rarity image', async () => {
    const draft: ItemDraftPayload = {
      id: 'new',
      name: 'Shirt',
      type: ItemType.WEARABLE,
      bodyShape: BodyShapeType.BOTH,
      category: 'upper_body',
      rarity: 'epic',
      contents: { 'shirt.glb': bytes(2 * MB), 'thumbnail.png': bytes(200_000) },
      model: 'shirt.glb',
      metrics: {},
      owner: '0xowner',
      collectionId: 'col-1'
    }
    const built = await buildItem(draft)
    const { bytes: total } = await measureItem(built, { sizeOf })
    const thumbnail = built.blobs['thumbnail.png'].size
    const image = built.blobs['image.png'].size
    // The thumbnail and the rarity image are their own files on the catalyst; here (no canvas) they may share bytes.
    const pictures =
      built.item.contents['image.png'] === built.item.contents['thumbnail.png'] ? thumbnail : thumbnail + image
    expect(total).toBe(2 * MB + pictures)
    expect(sizeOf).not.toHaveBeenCalled()
  })

  it('still counts the other body shape, the thumbnail and the rarity image when one shape is replaced', async () => {
    // The legacy Builder measured only the new file here, so a 1.5 MB save passed for a 3.09 MB item.
    const built = await withReplacedRepresentation(storedItem(), BodyShapeType.MALE, {
      contents: { 'shirt_v2.glb': bytes(BULL_AND_BEAR.male) },
      model: 'shirt_v2.glb',
      metrics: {}
    })
    const size = await measureItem(built, { sizeOf })
    expect(size.bytes).toBe(3_243_672)
    expect(isOverSizeLimit(size)).toBe(true)
    expect(sizeOf).not.toHaveBeenCalledWith('bafmale')
  })

  it('gives skins 9 MB for the same files', async () => {
    const skin = storedItem({ data: { ...storedItem().data, category: 'skin' } })
    const size = await measureItem({ item: skin }, { sizeOf })
    expect(size).toEqual({ bytes: 3_243_672, limit: 9 * MB })
    expect(isOverSizeLimit(size)).toBe(false)
  })

  it('leaves out the smart wearable preview video, which never deploys', async () => {
    const smart = storedItem({ contents: { ...storedItem().contents, 'video.mp4': 'bafvideo' } })
    expect((await measureItem({ item: smart }, { sizeOf })).bytes).toBe(3_243_672)
  })

  it('counts the rarity image a legacy item without one gets at deploy', async () => {
    const withoutImage = Object.fromEntries(
      Object.entries(storedItem().contents).filter(([path]) => path !== 'image.png')
    )
    const legacy = storedItem({ contents: withoutImage })
    const imageOf = vi.fn(async () => ({ hash: 'bafrendered', blob: bytes(90_000) }))
    const { bytes: total } = await measureItem({ item: legacy }, { sizeOf, imageOf })
    expect(total).toBe(BULL_AND_BEAR.male + BULL_AND_BEAR.female + BULL_AND_BEAR.thumbnail + 90_000)
  })
})

describe('linked items', () => {
  it('count only the files they deploy: never a generated rarity image', async () => {
    const withoutImage = Object.fromEntries(
      Object.entries(storedItem().contents).filter(([path]) => path !== 'image.png')
    )
    const linked = storedItem({
      urn: 'urn:decentraland:amoy:collections-thirdparty:brand:hats:bull-bear',
      contents: withoutImage
    })
    const imageOf = vi.fn(async () => ({ hash: 'bafrendered', blob: bytes(90_000) }))
    const { bytes: total } = await measureItem({ item: linked }, { sizeOf, imageOf })
    expect(total).toBe(BULL_AND_BEAR.male + BULL_AND_BEAR.female + BULL_AND_BEAR.thumbnail)
    expect(imageOf).not.toHaveBeenCalled()
  })
})

describe('changesItemSize', () => {
  const hat = { type: ItemType.WEARABLE, data: { category: 'hat' }, contents: {} } as unknown as Item
  const skin = { ...hat, data: { category: 'skin' } } as unknown as Item

  it('is true for a save that uploads files, and false for one that only uploads the preview video', () => {
    expect(changesItemSize({ item: hat, blobs: { 'thumbnail.png': new Blob(['x']) } }, hat)).toBe(true)
    expect(changesItemSize({ item: hat, blobs: { 'video.mp4': new Blob(['x']) } }, hat)).toBe(false)
  })

  it('is true for a change of category that moves the limit, and false for one that keeps it', () => {
    expect(changesItemSize({ item: hat }, skin)).toBe(true)
    expect(changesItemSize({ item: hat }, { ...hat, data: { category: 'mask' } } as unknown as Item)).toBe(false)
  })

  it('is false for an edit of the details alone', () => {
    expect(changesItemSize({ item: { ...hat, name: 'Cap' } }, hat)).toBe(false)
    expect(changesItemSize({ item: hat })).toBe(false)
  })
})

describe('formatItemSize', () => {
  it('rounds as the validator reports sizes, but never shows an item over its limit as at it', () => {
    expect(formatItemSize({ bytes: 3_093_299, limit: 3 * MB })).toEqual({ size: '2.95', limit: '3.00' })
    expect(formatItemSize({ bytes: 3 * MB, limit: 3 * MB })).toEqual({ size: '3.00', limit: '3.00' })
    expect(formatItemSize({ bytes: 3 * MB + 1, limit: 3 * MB })).toEqual({ size: '3.01', limit: '3.00' })
  })
})

describe('assertItemSize', () => {
  it('refuses an item over its limit with the creator-facing size and limit', async () => {
    await expect(assertItemSize({ item: storedItem() }, { sizeOf })).rejects.toMatchObject({
      messageKey: 'size_exceeded',
      messageParams: { size: '3.09', limit: '3.00' }
    })
    const skin = storedItem({ data: { ...storedItem().data, category: 'skin' } })
    await expect(assertItemSize({ item: skin }, { sizeOf })).resolves.toEqual({ bytes: 3_243_672, limit: 9 * MB })
  })
})

describe('guardItemSize', () => {
  it('refuses an item over its limit', async () => {
    await expect(guardItemSize({ item: storedItem() }, { sizeOf }, 'test')).rejects.toBeInstanceOf(ItemFileError)
  })

  it('lets the save through, reported, when storage cannot be measured', async () => {
    const unreachable = vi.fn(async () => Promise.reject(new Error('offline')))
    await expect(guardItemSize({ item: storedItem() }, { sizeOf: unreachable }, 'test')).resolves.toBeUndefined()
    expect(captureError).toHaveBeenCalledWith(expect.any(Error), expect.objectContaining({ flow: 'test' }))
  })
})
