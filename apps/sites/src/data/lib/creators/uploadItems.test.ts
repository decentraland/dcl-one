import { beforeEach, describe, expect, it, vi } from 'vitest'
import { BodyShapeType, ItemType } from './items'
import { EmotePlayMode, buildItem, type ItemDraftPayload } from './itemFactory'
import { BuilderServerError } from './builder'
import { type Item } from './items'
import { executeUpload, findDuplicateUrns, planDraftUpload, planUpload, type UploadDraft } from './uploadItems'

vi.mock('./builder', async importOriginal => {
  const original = await importOriginal<typeof import('./builder')>()
  return { ...original, saveItem: vi.fn(), fetchContentSize: vi.fn() }
})

const { saveItem, fetchContentSize } = await import('./builder')
const saveItemMock = vi.mocked(saveItem)
const fetchContentSizeMock = vi.mocked(fetchContentSize)

const blob = (text = 'x') => new Blob([text])

function draft(id: string, overrides: Partial<UploadDraft> = {}): UploadDraft {
  return {
    id,
    name: `Item ${id}`,
    type: ItemType.WEARABLE,
    bodyShape: BodyShapeType.BOTH,
    category: 'hat',
    rarity: 'epic',
    playMode: EmotePlayMode.SIMPLE,
    contents: { 'model.glb': blob(id), 'thumbnail.png': blob('thumb') },
    model: 'model.glb',
    metrics: { triangles: 10 },
    owner: '0xowner',
    collectionId: 'col-1',
    ...overrides
  }
}

beforeEach(() => {
  saveItemMock.mockReset()
  saveItemMock.mockResolvedValue({} as never)
  fetchContentSizeMock.mockReset()
  fetchContentSizeMock.mockResolvedValue(6)
})

describe('planUpload', () => {
  it('creates one operation per base draft', async () => {
    const operations = await planUpload([draft('a'), draft('b')], [])
    expect(operations).toHaveLength(2)
    expect(operations.map(operation => operation.draftIds)).toEqual([['a'], ['b']])
  })

  it('merges a variant of a batch draft into its base operation', async () => {
    const base = draft('base', { bodyShape: BodyShapeType.MALE })
    const variant = draft('variant', {
      bodyShape: BodyShapeType.FEMALE,
      variantTargetId: 'base',
      contents: { 'f.glb': blob('f'), 'thumbnail.png': blob('t') },
      model: 'f.glb'
    })
    const operations = await planUpload([base, variant], [])
    expect(operations).toHaveLength(1)
    expect(operations[0].draftIds).toEqual(['base', 'variant'])
    expect(operations[0].built.item.data.representations).toHaveLength(2)
  })

  it('appends a variant of an existing item without re-uploading its thumbnail', async () => {
    const { item: existing } = await buildItem({
      ...(draft('existing') as ItemDraftPayload),
      bodyShape: BodyShapeType.MALE
    })
    const variant = draft('variant', {
      bodyShape: BodyShapeType.FEMALE,
      variantTargetId: existing.id,
      contents: { 'f.glb': blob('f'), 'thumbnail.png': blob('t') },
      model: 'f.glb'
    })
    const operations = await planUpload([variant], [existing])
    expect(operations).toHaveLength(1)
    expect(operations[0].isExistingItemUpdate).toBe(true)
    expect(Object.keys(operations[0].built.blobs)).toEqual(['female/f.glb'])
  })

  it('rejects a variant whose target is nowhere to be found', async () => {
    await expect(
      planUpload([draft('v', { variantTargetId: 'ghost', bodyShape: BodyShapeType.MALE })], [])
    ).rejects.toThrow()
  })
})

describe('executeUpload', () => {
  it('reports every draft as saved on success', async () => {
    const operations = await planUpload([draft('a'), draft('b')], [])
    const result = await executeUpload('0xowner', operations)
    expect(result).toEqual({ savedDraftIds: ['a', 'b'], failedDraftIds: [], failureReason: null })
    expect(saveItemMock).toHaveBeenCalledTimes(2)
  })

  it('keeps uploading after a generic failure and reports partial results', async () => {
    saveItemMock.mockRejectedValueOnce(new BuilderServerError('boom', 500))
    const operations = await planUpload([draft('a'), draft('b')], [])
    const result = await executeUpload('0xowner', operations)
    expect(result.savedDraftIds).toEqual(['b'])
    expect(result.failedDraftIds).toEqual(['a'])
    expect(result.failureReason).toBe('generic')
  })

  it('aborts on a locked collection and fails everything left', async () => {
    saveItemMock.mockRejectedValueOnce(new BuilderServerError('locked', 423))
    const operations = await planUpload([draft('a'), draft('b'), draft('c')], [])
    const result = await executeUpload('0xowner', operations)
    expect(result.savedDraftIds).toEqual([])
    expect(result.failedDraftIds).toEqual(['a', 'b', 'c'])
    expect(result.failureReason).toBe('locked')
    expect(saveItemMock).toHaveBeenCalledTimes(1)
  })

  it('refuses a representation that would push the stored item over its cap, and keeps uploading', async () => {
    const { item: existing } = await buildItem({
      ...(draft('existing') as ItemDraftPayload),
      bodyShape: BodyShapeType.MALE
    })
    const variant = draft('variant', {
      bodyShape: BodyShapeType.FEMALE,
      variantTargetId: existing.id,
      contents: { 'f.glb': blob('f'), 'thumbnail.png': blob('t') },
      model: 'f.glb'
    })
    // Stored model + retained thumbnail together cross the cap; neither alone does.
    fetchContentSizeMock.mockImplementation(async (hash: string) =>
      hash === existing.contents['thumbnail.png'] ? 1024 * 1024 : 2.5 * 1024 * 1024
    )
    const operations = await planUpload([variant, draft('b')], [existing])
    const result = await executeUpload('0xowner', operations)
    expect(result.failedDraftIds).toEqual(['variant'])
    expect(result.savedDraftIds).toEqual(['b'])
    expect(result.failureReason).toBe('too_big')
    expect(saveItemMock).toHaveBeenCalledTimes(1)
  })

  it('counts a stored file referenced at several paths once', async () => {
    const { item: built } = await buildItem({
      ...(draft('existing') as ItemDraftPayload),
      bodyShape: BodyShapeType.MALE
    })
    const modelHash = built.contents['male/model.glb']
    const existing = { ...built, contents: { ...built.contents, 'male/copy.glb': modelHash } }
    const variant = draft('variant', {
      bodyShape: BodyShapeType.FEMALE,
      variantTargetId: existing.id,
      contents: { 'f.glb': blob('f'), 'thumbnail.png': blob('t') },
      model: 'f.glb'
    })
    // 2MB once fits; counted per path it would not.
    fetchContentSizeMock.mockImplementation(async (hash: string) => (hash === modelHash ? 2 * 1024 * 1024 : 5))
    const result = await executeUpload('0xowner', await planUpload([variant], [existing]))
    expect(result.savedDraftIds).toEqual(['variant'])
  })

  it('refuses a new item whose model fits but whose thumbnail takes it over the limit', async () => {
    const big = draft('big', {
      contents: {
        'model.glb': new Blob([new Uint8Array(3 * 1024 * 1024 - 100)]),
        'thumbnail.png': blob('t'.repeat(200))
      }
    })
    const result = await executeUpload('0xowner', await planUpload([big, draft('b')], []))
    expect(result.failedDraftIds).toEqual(['big'])
    expect(result.savedDraftIds).toEqual(['b'])
    expect(result.failureReason).toBe('too_big')
  })

  it('maps an already-published conflict to its own reason', async () => {
    saveItemMock.mockRejectedValue(new BuilderServerError('published', 409))
    const operations = await planUpload([draft('a')], [])
    const result = await executeUpload('0xowner', operations)
    expect(result.failureReason).toBe('published')
  })

  it("tells a URN another item holds apart from a published collection, even when the message says 'return'", async () => {
    saveItemMock.mockRejectedValue(
      new BuilderServerError("The URN provided already belong to another item. The item can't be inserted.", 409)
    )
    expect((await executeUpload('0xowner', await planUpload([draft('a')], []))).failureReason).toBe('urn_taken')
    saveItemMock.mockRejectedValue(new BuilderServerError('Items cannot be returned once published', 409))
    expect((await executeUpload('0xowner', await planUpload([draft('a')], []))).failureReason).toBe('published')
  })
})

describe('planDraftUpload', () => {
  it('builds a variant inside the batch draft it belongs to, as the upload will', async () => {
    const base = draft('base', { bodyShape: BodyShapeType.MALE })
    const variant = draft('variant', {
      bodyShape: BodyShapeType.FEMALE,
      variantTargetId: 'base',
      contents: { 'f.glb': blob('f'), 'thumbnail.png': blob('t') },
      model: 'f.glb'
    })
    const operation = await planDraftUpload('variant', [base, variant, draft('other')], [])
    expect(operation?.draftIds).toEqual(['base', 'variant'])
    expect(Object.keys(operation!.built.item.contents)).toEqual(
      expect.arrayContaining(['male/model.glb', 'female/f.glb', 'thumbnail.png', 'image.png'])
    )
  })

  it("counts a batch variant's files in the draft it targets, too", async () => {
    const base = draft('base', { bodyShape: BodyShapeType.MALE })
    const variant = draft('variant', {
      bodyShape: BodyShapeType.FEMALE,
      variantTargetId: 'base',
      contents: { 'f.glb': blob('f') },
      model: 'f.glb'
    })
    const operation = await planDraftUpload('base', [variant, base], [])
    expect(operation?.draftIds).toEqual(['base', 'variant'])
    expect(operation?.built.blobs['female/f.glb']).toBeDefined()
  })

  it('builds a variant of a saved item on top of that item', async () => {
    const { item: existing } = await buildItem({
      ...(draft('existing') as ItemDraftPayload),
      bodyShape: BodyShapeType.MALE
    })
    const variant = draft('variant', {
      bodyShape: BodyShapeType.FEMALE,
      variantTargetId: existing.id,
      contents: { 'f.glb': blob('f') },
      model: 'f.glb'
    })
    const operation = await planDraftUpload('variant', [variant], [existing])
    expect(operation?.isExistingItemUpdate).toBe(true)
    expect(operation?.built.item.contents['male/model.glb']).toBe(existing.contents['male/model.glb'])
    expect(Object.keys(operation!.built.blobs)).toEqual(['female/f.glb'])
  })
})

describe('linked URNs', () => {
  const URN = 'urn:decentraland:amoy:collections-thirdparty:brand:hats:cap'
  const existing = { id: 'item-1', urn: URN, owner: '', createdAt: 1, contents: {} } as unknown as Item

  it('only replaces the item that holds a URN when editing in bulk', async () => {
    const [update] = await planUpload([draft('a', { urn: URN })], [existing], { replaceByUrn: true })
    expect(update.isExistingItemUpdate).toBe(true)
    expect(update.built.item.id).toBe('item-1')

    const [added] = await planUpload([draft('a', { urn: URN })], [existing])
    expect(added.isExistingItemUpdate).toBe(false)
    expect(added.built.item.id).not.toBe('item-1')
  })

  it('finds the drafts that repeat a URN an earlier draft of the batch takes', () => {
    const drafts = [
      draft('a', { urn: URN }),
      draft('b', { urn: `${URN}-2` }),
      draft('c', { urn: URN }),
      draft('v', { urn: URN, variantTargetId: 'a' })
    ]
    expect(findDuplicateUrns(drafts)).toEqual(['c'])
    expect(findDuplicateUrns([draft('x'), draft('y')])).toEqual([])
  })
})
