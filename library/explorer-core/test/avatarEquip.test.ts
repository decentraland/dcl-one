import { describe, it, expect } from 'vitest'
import { lookDeploy, sameLook, type AvatarLook } from '../engine/avatarEquip'

const look = (over: Partial<AvatarLook> = {}): AvatarLook => ({
  bodyShape: 'urn:body:a',
  eyes: { r: 0, g: 0, b: 1 },
  hair: { r: 1, g: 0, b: 0 },
  skin: { r: 1, g: 1, b: 1 },
  wearables: ['urn:hat'],
  emotes: ['wave', '', '', '', '', '', '', '', '', ''],
  forceRender: ['hair'],
  ...over
})

// The Backpack deploys its look on close, in one setAvatar.
describe('look deploy', () => {
  // Audit backpack-emotes-2: every wearable/emote equip sent forceRender: [], erasing overrides.
  it('keeps force-render overrides with the equipped set', () => {
    expect(lookDeploy(look({ wearables: ['urn:cap'] }), look()).equip).toEqual({
      wearableUrns: ['urn:cap'],
      emoteUrns: ['wave', '', '', '', '', '', '', '', '', ''],
      forceRender: ['hair']
    })
  })

  it('updates body and colors without changing the name', () => {
    expect(lookDeploy(look({ wearables: [] }), look()).base).toBeUndefined()
    const patch = lookDeploy(look({ hair: { r: 0, g: 1, b: 0 } }), look())
    expect(patch.preserveName).toBe(true)
    expect(patch.base).toEqual({
      name: '',
      bodyShapeUrn: 'urn:body:a',
      eyesColor: { r: 0, g: 0, b: 1 },
      hairColor: { r: 0, g: 1, b: 0 },
      skinColor: { r: 1, g: 1, b: 1 },
      talking: null
    })
  })

  it('compares looks by value', () => {
    expect(sameLook(look(), look())).toBe(true)
    expect(sameLook(look(), look({ forceRender: [] }))).toBe(false)
  })
})
