import { describe, expect, it, vi } from 'vitest'
import { BodyShape, PreviewEmote, type EmoteWithBlobs, type WearableWithBlobs } from '@dcl/schemas'
import {
  buildPreviewOptions,
  definitionToBase64,
  getPreviewZoom,
  itemToDefinition,
  retryWhilePreviewReloads,
  type UnsavedFiles
} from './preview'
import { ItemType, type Item } from './items'
import { type AvatarAttributes } from './avatar'

const wearable: Item = {
  id: 'w1',
  name: 'Hat',
  description: 'A hat',
  thumbnail: 'thumbnail.png',
  owner: '0xabc',
  isPublished: false,
  isApproved: false,
  inCatalyst: false,
  type: ItemType.WEARABLE,
  data: {
    category: 'hat',
    hides: ['hair'],
    representations: [{ bodyShapes: [BodyShape.MALE], mainFile: 'male/hat.glb', contents: ['male/hat.glb'] }]
  },
  contents: { 'male/hat.glb': 'bafyglb', 'thumbnail.png': 'bafythumb' },
  createdAt: 1,
  updatedAt: 1
}

const emote: Item = {
  ...wearable,
  id: 'e1',
  type: ItemType.EMOTE,
  data: {
    category: 'dance',
    loop: true,
    representations: [
      { bodyShapes: [BodyShape.MALE, BodyShape.FEMALE], mainFile: 'male/dance.glb', contents: ['male/dance.glb'] }
    ]
  },
  contents: { 'male/dance.glb': 'bafyemote' }
}

const avatar: AvatarAttributes = {
  bodyShape: BodyShape.FEMALE,
  skin: 'ffe4c6',
  eyes: '362626',
  hair: '1c1c1c',
  baseWearableUrns: ['urn:hair'],
  hasOutfit: true
}

describe('itemToDefinition', () => {
  it('resolves every content path to its storage url and keeps the item id', () => {
    const definition = itemToDefinition(wearable)
    expect(definition.id).toBe('w1')
    expect('data' in definition && definition.data.representations[0].contents).toEqual([
      { key: 'male/hat.glb', url: 'https://builder-api.decentraland.zone/v1/storage/contents/bafyglb' }
    ])
    expect('data' in definition && definition.data.hides).toEqual(['hair'])
  })

  it('builds an emote definition with its loop flag', () => {
    const definition = itemToDefinition(emote)
    expect('emoteDataADR74' in definition && definition.emoteDataADR74.loop).toBe(true)
  })

  it('encodes definitions the way the preview url expects', () => {
    const definition = itemToDefinition(wearable)
    expect(JSON.parse(atob(definitionToBase64(definition)))).toEqual(definition)
  })
})

describe('buildPreviewOptions', () => {
  it('sends the avatar, the dressed items and the chosen animation', () => {
    const options = buildPreviewOptions({ kind: 'items', items: [wearable] }, avatar, PreviewEmote.DANCE)
    expect(options).toMatchObject({
      bodyShape: BodyShape.FEMALE,
      skin: 'ffe4c6',
      urns: ['urn:hair'],
      emote: PreviewEmote.DANCE,
      disableDefaultWearables: true,
      disableDefaultEmotes: false
    })
    expect(options.base64s).toHaveLength(1)
    expect(options.zoom).toBe(getPreviewZoom(PreviewEmote.DANCE))
  })

  it('draws the scene boundaries only when asked, so a toggle reaches the mounted iframe', () => {
    expect(buildPreviewOptions({ kind: 'items', items: [] }, avatar, PreviewEmote.IDLE).showSceneBoundaries).toBe(false)
    expect(buildPreviewOptions({ kind: 'items', items: [] }, avatar, PreviewEmote.IDLE, true).showSceneBoundaries).toBe(
      true
    )
  })

  it('lets an emote item own the animation and frames the jump further out', () => {
    const withEmote = buildPreviewOptions({ kind: 'items', items: [wearable, emote] }, avatar, PreviewEmote.DANCE)
    expect(withEmote.disableDefaultEmotes).toBe(true)
    expect(withEmote.emote).toBeUndefined()
    const jump = buildPreviewOptions({ kind: 'items', items: [] }, avatar, PreviewEmote.JUMP).zoom!
    const dance = buildPreviewOptions({ kind: 'items', items: [] }, avatar, PreviewEmote.DANCE).zoom!
    // A lower zoom is a larger camera radius: the jump has to fit in frame.
    expect(jump).toBeLessThan(dance)
  })

  it('keeps the default outfit until one is picked, then leaves emptied slots empty', () => {
    const items = { kind: 'items' as const, items: [] }
    expect(buildPreviewOptions(items, { ...avatar, hasOutfit: false }, PreviewEmote.IDLE).disableDefaultWearables).toBe(
      false
    )
    expect(buildPreviewOptions(items, avatar, PreviewEmote.IDLE).disableDefaultWearables).toBe(true)
  })

  describe('with a model picked but not saved yet', () => {
    const replaced: Item = {
      ...wearable,
      data: {
        ...wearable.data,
        representations: [
          { bodyShapes: [BodyShape.MALE, BodyShape.FEMALE], mainFile: 'male/cap.glb', contents: ['male/cap.glb'] }
        ]
      },
      contents: { 'male/cap.glb': 'bafycap', 'thumbnail.png': 'bafythumb' }
    }
    const blobs = { 'male/cap.glb': new Blob(['cap']) }

    it('sends the new model as blobs, with the item data, instead of storage URLs that would 404', () => {
      const options = buildPreviewOptions(
        { kind: 'items', items: [replaced, emote], unsaved: { itemId: replaced.id, blobs } },
        avatar,
        PreviewEmote.DANCE
      )
      expect(options.base64s).toEqual([definitionToBase64(itemToDefinition(emote))])
      const blob = options.blob as WearableWithBlobs
      expect(blob.data.category).toBe('hat')
      expect(blob.data.representations[0].contents).toEqual([{ key: 'male/cap.glb', blob: blobs['male/cap.glb'] }])
    })

    it('tells two picked files apart, and the saved item apart from both', () => {
      const send = (item: Item, unsaved?: UnsavedFiles) =>
        JSON.stringify(buildPreviewOptions({ kind: 'items', items: [item], unsaved }, avatar, PreviewEmote.IDLE))
      const other: Item = { ...replaced, contents: { ...replaced.contents, 'male/cap.glb': 'bafyother' } }
      const first = send(replaced, { itemId: replaced.id, blobs })
      const second = send(other, { itemId: other.id, blobs: { 'male/cap.glb': new Blob(['other']) } })
      expect(first).not.toBe(second)
      expect(send(other)).not.toBe(second)
    })

    it('sends an unsaved emote as blobs too', () => {
      const dance = { 'male/dance.glb': new Blob(['dance']) }
      const options = buildPreviewOptions(
        { kind: 'items', items: [emote], unsaved: { itemId: emote.id, blobs: dance } },
        avatar,
        PreviewEmote.IDLE
      )
      expect(options.base64s).toEqual([])
      const blob = options.blob as EmoteWithBlobs
      expect(blob.emoteDataADR74.loop).toBe(true)
      expect(blob.emoteDataADR74.representations[0].contents).toEqual([
        { key: 'male/dance.glb', blob: dance['male/dance.glb'] }
      ])
    })

    it('keeps showing the stored representation when the new one is for the other body shape', () => {
      const withFemale: Item = {
        ...wearable,
        data: {
          ...wearable.data,
          representations: [
            ...wearable.data.representations,
            { bodyShapes: [BodyShape.FEMALE], mainFile: 'female/hat.glb', contents: ['female/hat.glb'] }
          ]
        },
        contents: { ...wearable.contents, 'female/hat.glb': 'bafyfemale' }
      }
      const unsaved = { itemId: wearable.id, blobs: { 'female/hat.glb': new Blob(['hat']) } }
      const male = buildPreviewOptions(
        { kind: 'items', items: [withFemale], unsaved },
        { ...avatar, bodyShape: BodyShape.MALE },
        PreviewEmote.IDLE
      )
      expect(male.blob).toBeUndefined()
      expect(male.base64s).toEqual([definitionToBase64(itemToDefinition(wearable))])
      const female = buildPreviewOptions({ kind: 'items', items: [withFemale], unsaved }, avatar, PreviewEmote.IDLE)
      expect(female.base64s).toEqual([])
      expect((female.blob as WearableWithBlobs).data.representations[0].mainFile).toBe('female/hat.glb')
    })
  })
})

describe('retryWhilePreviewReloads', () => {
  const reloading = () => new Error('The preview is reloading')

  it('asks again until the renderer has finished reloading', async () => {
    const query = vi.fn().mockRejectedValueOnce(reloading()).mockRejectedValueOnce(reloading()).mockResolvedValue(42)
    await expect(retryWhilePreviewReloads(query, { delayMs: 1 })).resolves.toBe(42)
    expect(query).toHaveBeenCalledTimes(3)
  })

  it('asks again when a reload overtook the capture', async () => {
    const query = vi
      .fn()
      .mockRejectedValueOnce(new Error('The preview reloaded during the capture'))
      .mockResolvedValue('data:image/png;base64,')
    await expect(retryWhilePreviewReloads(query, { delayMs: 1 })).resolves.toBe('data:image/png;base64,')
  })

  it('fails right away on any other error', async () => {
    const query = vi.fn().mockRejectedValue(new Error('The scene was disposed'))
    await expect(retryWhilePreviewReloads(query, { delayMs: 1 })).rejects.toThrow('The scene was disposed')
    expect(query).toHaveBeenCalledTimes(1)
  })

  it('gives up once the renderer has been reloading for too long', async () => {
    const query = vi.fn().mockRejectedValue(reloading())
    await expect(retryWhilePreviewReloads(query, { delayMs: 5, maxWaitMs: 20 })).rejects.toThrow(
      'The preview is reloading'
    )
  })
})
