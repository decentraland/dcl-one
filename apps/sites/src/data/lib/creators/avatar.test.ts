import { describe, expect, it } from 'vitest'
import { BodyShape, WearableCategory } from '@dcl/schemas'
import {
  EYE_COLORS,
  HAIR_COLORS,
  SKIN_COLORS,
  filterBaseWearables,
  getBaseWearableName,
  getRandomBaseWearables,
  getRandomColors,
  rgbToHex,
  toBaseWearableUrns,
  type BaseWearable
} from './avatar'

const catalog: BaseWearable[] = [
  { urn: 'urn:hair_m', category: WearableCategory.HAIR, bodyShapes: [BodyShape.MALE], name: 'Hair' },
  { urn: 'urn:hair_f', category: WearableCategory.HAIR, bodyShapes: [BodyShape.FEMALE], name: 'Hair' },
  {
    urn: 'urn:beard',
    category: WearableCategory.FACIAL_HAIR,
    bodyShapes: [BodyShape.MALE, BodyShape.FEMALE],
    name: 'Beard'
  },
  {
    urn: 'urn:shirt',
    category: WearableCategory.UPPER_BODY,
    bodyShapes: [BodyShape.MALE, BodyShape.FEMALE],
    name: 'Shirt'
  },
  {
    urn: 'urn:pants',
    category: WearableCategory.LOWER_BODY,
    bodyShapes: [BodyShape.MALE, BodyShape.FEMALE],
    name: 'Pants'
  }
]

describe('avatar palettes', () => {
  it('converts the legacy float colors to preview hex strings', () => {
    expect(rgbToHex([1, 0.8941177, 0.7764706])).toBe('ffe4c6')
    expect(SKIN_COLORS[0]).toBe('ffe4c6')
    expect(HAIR_COLORS).toHaveLength(8)
    expect(EYE_COLORS.every(hex => /^[0-9a-f]{6}$/.test(hex))).toBe(true)
    const colors = getRandomColors(() => 0)
    expect(colors).toEqual({ skin: SKIN_COLORS[0], eyes: EYE_COLORS[0], hair: HAIR_COLORS[0] })
  })
})

describe('base wearables', () => {
  it('offers only the slot entries that exist for the body shape', () => {
    expect(filterBaseWearables(catalog, WearableCategory.HAIR, BodyShape.FEMALE).map(w => w.urn)).toEqual([
      'urn:hair_f'
    ])
  })

  it('dresses a random outfit per slot, without facial hair for the female shape', () => {
    expect(getRandomBaseWearables(catalog, BodyShape.MALE, () => 0)).toEqual({
      hair: 'urn:hair_m',
      facial_hair: 'urn:beard',
      upper_body: 'urn:shirt',
      lower_body: 'urn:pants'
    })
    expect(getRandomBaseWearables(catalog, BodyShape.FEMALE, () => 0).facial_hair).toBeNull()
  })

  it('lists the picked urns, skipping empty slots, plus the fixed shoes for the body shape', () => {
    expect(
      toBaseWearableUrns({ hair: 'urn:a', facial_hair: null, upper_body: 'urn:b', lower_body: null }, BodyShape.MALE)
    ).toEqual(['urn:a', 'urn:b', 'urn:decentraland:off-chain:base-avatars:sneakers'])
    expect(toBaseWearableUrns(null, BodyShape.FEMALE)).toEqual(['urn:decentraland:off-chain:base-avatars:bun_shoes'])
  })

  it('names catalog entries from their urn like the legacy editor', () => {
    expect(getBaseWearableName('urn:decentraland:off-chain:base-avatars:f_jeans_00')).toBe('Jeans')
    expect(getBaseWearableName('urn:decentraland:off-chain:base-avatars:hair_02')).toBe('Hair 3')
    expect(getBaseWearableName('urn:decentraland:off-chain:base-avatars:green_hoodie')).toBe('Green Hoodie')
  })
})
