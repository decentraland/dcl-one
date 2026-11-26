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
    expect(patch.equip).toBeUndefined()
    expect(patch.base).toEqual({
      name: '',
      bodyShapeUrn: '',
      eyesColor: null,
      hairColor: { r: 0, g: 1, b: 0 },
      skinColor: null,
      talking: null
    })
  })

  it('submits the fitting equipped set together with a changed body', () => {
    const changed = look({ bodyShape: 'urn:body:b' })
    expect(lookDeploy(changed, look())).toMatchObject({
      base: { bodyShapeUrn: 'urn:body:b', eyesColor: null, hairColor: null, skinColor: null },
      equip: { wearableUrns: changed.wearables, emoteUrns: changed.emotes, forceRender: changed.forceRender }
    })
  })

  it('compares looks by value', () => {
    expect(sameLook(look(), look())).toBe(true)
    expect(sameLook(look(), look({ forceRender: [] }))).toBe(false)
  })
})
