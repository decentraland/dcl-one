import { describe, expect, it } from 'vitest'
import {
  getHideableBodyPartCategories,
  getHideableWearableCategories,
  getWearableCategoryOptions
} from './wearableCategories'

const model = { 'male/hat.glb': 'hash' }
const texture = { 'eyes.png': 'hash' }

describe('wearable categories', () => {
  it('offers texture slots to PNG wearables and every other slot but body shape to models', () => {
    expect(getWearableCategoryOptions(texture)).toEqual(['eyebrows', 'eyes', 'mouth'])
    const options = getWearableCategoryOptions(model)
    expect(options).toContain('hat')
    expect(options).not.toContain('body_shape')
    expect(options).not.toContain('eyes')
  })

  it('lets only model wearables hide body parts', () => {
    expect(getHideableBodyPartCategories(model)).toEqual(['head', 'hands'])
    expect(getHideableBodyPartCategories(texture)).toEqual([])
  })

  it('never offers body_shape anew but keeps it while already hidden', () => {
    expect(getHideableWearableCategories(model, 'hat')).not.toContain('body_shape')
    expect(getHideableWearableCategories(model, 'hat', ['body_shape'])).toContain('body_shape')
  })

  it("never offers a wearable's own category anew but keeps it while already hidden", () => {
    expect(getHideableWearableCategories(model, 'hat')).not.toContain('hat')
    expect(getHideableWearableCategories(model, 'hat', ['hat'])).toContain('hat')
  })

  it('stops a skin from hiding what it already covers', () => {
    const hideable = getHideableWearableCategories(model, 'skin')
    expect(hideable).not.toContain('upper_body')
    expect(hideable).not.toContain('skin')
    expect(hideable).toContain('hat')
  })
})
