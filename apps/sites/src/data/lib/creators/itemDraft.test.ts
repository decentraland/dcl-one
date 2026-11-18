import { describe, expect, it, vi } from 'vitest'
import { BodyShape } from '@dcl/schemas'
import {
  applyDraftToItem,
  toPreviewItem,
  isValidItemDescription,
  sanitizeItemDescription,
  tracksVideoOnSave,
  computeRemovesDefaultHiding,
  createItemDraft,
  isItemDraftDirty,
  itemDraftReducer,
  toFileChangeSaveable,
  toSaveableItem
} from './itemDraft'
import { ItemType, type Item } from './items'

const item: Item = {
  id: 'w1',
  name: 'Hat',
  description: 'A hat',
  thumbnail: 'thumbnail.png',
  owner: '0xabc',
  rarity: 'epic',
  isPublished: false,
  isApproved: false,
  inCatalyst: false,
  type: ItemType.WEARABLE,
  data: {
    category: 'hat',
    hides: ['hair'],
    replaces: ['helmet'],
    tags: ['cool'],
    representations: [
      {
        bodyShapes: [BodyShape.MALE],
        mainFile: 'male/hat.glb',
        contents: ['male/hat.glb'],
        overrideHides: [],
        overrideReplaces: []
      }
    ]
  },
  contents: { 'male/hat.glb': 'bafyglb', 'thumbnail.png': 'bafythumb', 'video.mp4': 'bafyvideo' },
  createdAt: 1,
  updatedAt: 1
}

describe('item draft', () => {
  it('starts from the item, folding legacy replaces into hides', () => {
    const draft = createItemDraft(item)
    expect(draft.hides).toEqual(['hair', 'helmet'])
    expect(draft.outlineCompatible).toBe(true)
    expect(isItemDraftDirty(draft, item)).toBe(false)
  })

  it('tracks edits and picked files as dirty', () => {
    const draft = createItemDraft(item)
    expect(isItemDraftDirty(itemDraftReducer(draft, { type: 'setText', field: 'name', value: 'Cap' }), item)).toBe(true)
    expect(isItemDraftDirty(itemDraftReducer(draft, { type: 'setThumbnail', thumbnail: new Blob(['x']) }), item)).toBe(
      true
    )
    expect(isItemDraftDirty(itemDraftReducer(draft, { type: 'setTags', tags: [' cool ', 'cool'] }), item)).toBe(false)
  })

  it('clears the hides when the wearable becomes a skin', () => {
    const draft = itemDraftReducer(createItemDraft(item), { type: 'setCategory', category: 'skin' })
    expect(draft.hides).toEqual([])
    expect(itemDraftReducer(createItemDraft(item), { type: 'setCategory', category: 'helmet' }).hides).toEqual(['hair'])
  })

  it('saves a file change from the saved item alone, with the render and the category it took', async () => {
    const update = {
      item: {
        ...item,
        data: {
          ...item.data,
          representations: [{ bodyShapes: [BodyShape.MALE], mainFile: 'male/eyes.png', contents: ['male/eyes.png'] }]
        },
        contents: { 'male/eyes.png': 'bafyeyes', 'thumbnail.png': 'bafythumb', 'video.mp4': 'bafyvideo' }
      },
      blobs: { 'male/eyes.png': new Blob(['png']) }
    }
    const springBones = { version: 1, models: { bafyeyes: {} } }
    const built = await toFileChangeSaveable(
      item,
      { update, thumbnail: new Blob(['render']), category: 'eyes' },
      { springBones }
    )
    expect(built.item.name).toBe('Hat')
    expect(built.item.data.springBones).toEqual(springBones)
    expect(built.item.data.category).toBe('eyes')
    expect(built.item.data.hides).toEqual([])
    expect(built.item.data.representations[0].mainFile).toBe('male/eyes.png')
    expect(built.item.contents['thumbnail.png']).not.toBe('bafythumb')
    expect(Object.keys(built.blobs).sort()).toEqual(['image.png', 'male/eyes.png', 'thumbnail.png'])
  })

  it('projects the draft onto the item with mirrored overrides and default hiding', () => {
    let draft = createItemDraft(item)
    draft = itemDraftReducer(draft, { type: 'setCategory', category: 'upper_body' })
    draft = itemDraftReducer(draft, { type: 'setText', field: 'utility', value: ' glows ' })
    const next = applyDraftToItem(item, draft)
    expect(next.data.category).toBe('upper_body')
    expect(next.data.replaces).toEqual([])
    expect(next.data.removesDefaultHiding).toEqual(['hands'])
    expect(next.data.representations[0].overrideHides).toEqual(['hair', 'helmet'])
    expect(next.utility).toBe('glows')
    expect(computeRemovesDefaultHiding('hat', ['upper_body'])).toEqual(['hands'])
    expect(computeRemovesDefaultHiding('hat', [])).toEqual([])
  })

  it('saves and clears the utility of an emote too', () => {
    const emote: Item = {
      ...item,
      type: ItemType.EMOTE,
      utility: 'waves',
      data: { category: 'fun', tags: [], loop: false, representations: item.data.representations }
    }
    let draft = itemDraftReducer(createItemDraft(emote), { type: 'setText', field: 'utility', value: ' greets ' })
    expect(isItemDraftDirty(draft, emote)).toBe(true)
    expect(applyDraftToItem(emote, draft).utility).toBe('greets')

    draft = itemDraftReducer(draft, { type: 'setText', field: 'utility', value: '   ' })
    expect(applyDraftToItem(emote, draft)).not.toHaveProperty('utility')
  })

  it('keeps display-only edits out of what the renderer is handed', () => {
    let draft = createItemDraft(item)
    draft = itemDraftReducer(draft, { type: 'setText', field: 'name', value: 'Cap' })
    draft = itemDraftReducer(draft, { type: 'setText', field: 'description', value: 'New' })
    draft = itemDraftReducer(draft, { type: 'setTags', tags: ['new'] })
    const preview = toPreviewItem(item, draft)
    expect(preview.name).toBe(item.name)
    expect(preview.description).toBe(item.description)
    expect(preview.data.tags).toEqual(item.data.tags)
    // What the renderer does read still comes from the draft.
    draft = itemDraftReducer(draft, { type: 'setHides', hides: ['hat'] })
    expect(toPreviewItem(item, draft).data.hides).toEqual(['hat'])
  })

  it('hashes new files into the save and merges the spring bones', async () => {
    let draft = createItemDraft(item)
    draft = itemDraftReducer(draft, { type: 'setThumbnail', thumbnail: new Blob(['png'], { type: 'image/png' }) })
    draft = itemDraftReducer(draft, { type: 'setVideo', video: new Blob(['mp4']) })
    const springBones = {
      version: 1,
      models: {
        bafyglb: {
          Bone_springbone: {
            stiffness: 1,
            gravityPower: 0,
            gravityDir: [0, -1, 0] as [number, number, number],
            drag: 0.5
          }
        }
      }
    }
    const { item: saved, blobs } = await toSaveableItem(item, draft, { springBones })
    expect(Object.keys(blobs).sort()).toEqual(['image.png', 'thumbnail.png', 'video.mp4'])
    expect(saved.contents['thumbnail.png']).not.toBe('bafythumb')
    expect(saved.video).toBe(saved.contents['video.mp4'])
    expect(saved.video).not.toBe('bafyvideo')
    expect(saved.data.springBones).toEqual(springBones)
  })

  it('uploads a new video for an approved item but keeps the approved hash until curation copies it', async () => {
    const approved = { ...item, isPublished: true, isApproved: true, video: 'bafyvideo' }
    expect(tracksVideoOnSave(approved)).toBe(false)
    const draft = itemDraftReducer(createItemDraft(approved), { type: 'setVideo', video: new Blob(['mp4']) })
    const { item: saved, blobs } = await toSaveableItem(approved, draft)
    expect(blobs['video.mp4']).toBeDefined()
    expect(saved.contents['video.mp4']).not.toBe('bafyvideo')
    expect(saved.video).toBe('bafyvideo')
  })

  it('rebuilds the catalyst image from the stored thumbnail when only the rarity changes', async () => {
    const draft = itemDraftReducer(createItemDraft(item), { type: 'setRarity', rarity: 'legendary' })
    const fetchThumbnail = vi.fn().mockResolvedValue(new Blob(['png'], { type: 'image/png' }))
    const { item: saved, blobs } = await toSaveableItem(item, draft, { fetchThumbnail })
    expect(fetchThumbnail).toHaveBeenCalledWith(item)
    expect(Object.keys(blobs)).toEqual(['image.png'])
    expect(saved.rarity).toBe('legendary')
    expect(saved.contents['image.png']).toBeDefined()
    expect(saved.contents['thumbnail.png']).toBe('bafythumb')
  })

  it('saves the rarity change without an image when the stored thumbnail cannot be loaded', async () => {
    const draft = itemDraftReducer(createItemDraft(item), { type: 'setRarity', rarity: 'legendary' })
    const { item: saved, blobs } = await toSaveableItem(item, draft, { fetchThumbnail: async () => null })
    expect(saved.rarity).toBe('legendary')
    expect(blobs).toEqual({})
  })

  it('leaves the catalyst image alone when the rarity is unchanged', async () => {
    const draft = itemDraftReducer(createItemDraft(item), { type: 'setText', field: 'name', value: 'Cap' })
    const fetchThumbnail = vi.fn()
    const { blobs } = await toSaveableItem(item, draft, { fetchThumbnail })
    expect(fetchThumbnail).not.toHaveBeenCalled()
    expect(blobs).toEqual({})
  })

  it('rejects descriptions that would break the on-chain metadata', () => {
    expect(isValidItemDescription('')).toBe(true)
    expect(isValidItemDescription('A hat')).toBe(true)
    expect(isValidItemDescription('Note: red')).toBe(false)
    expect(isValidItemDescription('x'.repeat(65))).toBe(false)
    expect(sanitizeItemDescription('Note: ' + 'x'.repeat(70))).toBe('Note ' + 'x'.repeat(59))
    expect(sanitizeItemDescription('  A hat  ')).toBe('A hat')
  })
})
