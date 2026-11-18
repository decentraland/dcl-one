import { describe, expect, it } from 'vitest'
import { shopCollectionUrl, shopCreatorUrl, shopItemUrl } from './shop'

describe('shop urls', () => {
  it('points at the Shop item and collection pages of the current environment', () => {
    expect(shopItemUrl('0xabc', '3')).toBe('https://decentraland.zone/shop/item/0xabc/3')
    expect(shopCollectionUrl('0xabc')).toBe('https://decentraland.zone/shop/collection/0xabc')
    expect(shopCreatorUrl('0xABC')).toBe('https://decentraland.zone/shop/items/creator/0xabc')
  })
})
