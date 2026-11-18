import { describe, expect, it, vi } from 'vitest'
import JSZip from 'jszip'
import { type ContractNetwork } from '@dcl/schemas'
import { ItemType, type Item } from '~/lib/items'
import { ItemFileError } from '~/lib/itemFiles'
import { createDraft, type ItemDraft } from './AddItemsModal.state'
import { pickPreviewDraft, processDraftFile } from './processDraft'

const { analyzeModel } = vi.hoisted(() => ({ analyzeModel: vi.fn() }))
vi.mock('~/lib/models', () => ({ analyzeModel }))

function suggest(suggestedCategory: string | null) {
  analyzeModel.mockResolvedValue({ type: 'wearable', validationIssues: [], suggestedCategory })
}
suggest('upper_body')

function draft(overrides: Partial<ItemDraft> = {}): ItemDraft {
  return {
    ...createDraft(new File([new Blob(['x'])], 'hat.glb')),
    status: 'ready',
    type: ItemType.WEARABLE,
    contents: { 'model.glb': new Blob(['x']) },
    model: 'model.glb',
    ...overrides
  }
}

const done = { thumbnail: 'data:image/png;base64,x', metrics: { triangles: 1 } }

describe('pickPreviewDraft', () => {
  it('keeps the in-flight draft even when an earlier one becomes ready', () => {
    const drafts = [draft(), draft(), draft()]
    expect(pickPreviewDraft(drafts, drafts[0].id, drafts[2].id)).toBe(drafts[2])
  })

  it('prefers the selected draft over list order when nothing is in flight', () => {
    const drafts = [draft(), draft(), draft()]
    expect(pickPreviewDraft(drafts, drafts[1].id, null)).toBe(drafts[1])
  })

  it('moves on once the in-flight draft has its preview data', () => {
    const drafts = [draft(), draft(done), draft()]
    expect(pickPreviewDraft(drafts, drafts[1].id, drafts[1].id)).toBe(drafts[0])
  })

  it('finishes the type the preview was last mounted for before rebooting it for the other', () => {
    const finishedEmote = draft({ type: ItemType.EMOTE, ...done })
    const drafts = [draft(), finishedEmote, draft(), draft({ type: ItemType.EMOTE })]
    expect(pickPreviewDraft(drafts, finishedEmote.id, finishedEmote.id)).toBe(drafts[3])
  })

  it('skips drafts still being analyzed and returns null when everything is done', () => {
    const drafts = [draft({ status: 'processing', type: null }), draft(done)]
    expect(pickPreviewDraft(drafts, drafts[0].id, null)).toBeNull()
  })
})

describe('processDraftFile', () => {
  async function manifestZip({
    category = 'hat',
    hides = [],
    replaces = [],
    ...extra
  }: {
    category?: string
    hides?: string[]
    replaces?: string[]
    utility?: string
    id?: string
    mapping?: unknown
  } = {}): Promise<File> {
    const zip = new JSZip()
    zip.file(
      'wearable.json',
      JSON.stringify({
        name: 'Hat',
        ...extra,
        data: {
          category,
          replaces,
          hides,
          tags: [],
          representations: [
            {
              bodyShapes: ['urn:decentraland:off-chain:base-avatars:BaseMale'],
              mainFile: 'model.glb',
              contents: ['model.glb']
            }
          ]
        }
      })
    )
    zip.file('model.glb', 'glb')
    return new File([await zip.generateAsync({ type: 'arraybuffer' })], 'hat.zip')
  }

  it("keeps the hides a zip's wearable.json declares, unless the live preview set its own", async () => {
    expect((await processDraftFile(await manifestZip({ hides: ['hair'] }))).hides).toEqual(['hair'])
    expect((await processDraftFile(await manifestZip({ hides: ['hair'] }), ['head'])).hides).toEqual(['head'])
  })

  it("adds the slots a zip's wearable.json replaces to the hides, minus the item's own category", async () => {
    const draft = await processDraftFile(await manifestZip({ hides: ['hair', 'hat'], replaces: ['hair', 'earring'] }))
    expect(draft.hides).toEqual(['hair', 'earring'])
  })

  it("keeps a manifest category the model can take and falls back to the suggestion when it can't", async () => {
    expect(await processDraftFile(await manifestZip())).toMatchObject({ category: 'hat', validatedCategory: 'hat' })
    expect(
      await processDraftFile(await manifestZip({ category: 'eyes', hides: ['hair', 'upper_body'] }))
    ).toMatchObject({
      category: 'upper_body',
      validatedCategory: 'upper_body',
      hides: ['hair']
    })
  })

  it('leaves the category empty when the manifest category does not fit and nothing is suggested', async () => {
    suggest(null)
    const draft = await processDraftFile(await manifestZip({ category: 'eyes' }))
    suggest('upper_body')
    expect(draft.category).toBeUndefined()
    expect(draft.validatedCategory).toBeNull()
  })

  it("prefills the manifest's utility", async () => {
    expect((await processDraftFile(await manifestZip({ utility: ' Keeps you warm ' }))).utility).toBe('Keeps you warm')
  })

  describe('when editing a linked collection in bulk', () => {
    const contract = { network: 'amoy' as ContractNetwork, address: '0x' + '1'.repeat(40) }
    const collectionUrn = 'urn:decentraland:amoy:collections-thirdparty:brand:hats'
    const existing = {
      id: 'i1',
      urn: `${collectionUrn}:cap`,
      mappings: { amoy: { [contract.address]: [{ type: 'single', id: '7' }] } }
    } as unknown as Item
    const target = { collectionUrn, contract, updating: new Map([[existing.urn!, existing]]) }

    async function errorKey(file: File): Promise<string | undefined> {
      return processDraftFile(file, undefined, target).then(
        () => undefined,
        (error: unknown) => (error instanceof ItemFileError ? error.messageKey : 'unexpected')
      )
    }

    it('refuses a zip that names no item, or one this collection does not have', async () => {
      expect(await errorKey(await manifestZip())).toBe('bulk_edit_missing_urn')
      expect(await errorKey(await manifestZip({ id: `${collectionUrn}:boots` }))).toBe('bulk_edit_no_match')
    })

    it('updates the named item, keeping its mapping when the zip sets none', async () => {
      const draft = await processDraftFile(await manifestZip({ id: existing.urn }), undefined, target)
      expect(draft).toMatchObject({ urnSuffix: 'cap', mapping: { type: 'single', id: '7' } })
    })

    it('never keeps a preview video, which the content servers reject for linked items', async () => {
      const zip = new JSZip()
      const withVideo = await manifestZip({ id: existing.urn })
      await zip.loadAsync(await withVideo.arrayBuffer())
      zip.file('video.mp4', 'mp4')
      const file = new File([await zip.generateAsync({ type: 'arraybuffer' })], 'hat.zip')
      const draft = await processDraftFile(file, undefined, target)
      expect(draft.contents).not.toHaveProperty('video.mp4')
    })

    it("takes the zip's mapping when it has one", async () => {
      const draft = await processDraftFile(
        await manifestZip({ id: existing.urn, mapping: { type: 'range', from: '1', to: '9' } }),
        undefined,
        target
      )
      expect(draft.mapping).toEqual({ type: 'range', from: '1', to: '9' })
    })
  })
})
