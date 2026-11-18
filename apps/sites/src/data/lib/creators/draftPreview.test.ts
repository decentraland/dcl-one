import { describe, expect, it } from 'vitest'
import { applyPreviewResult, isThumbnailGenerating, type PreviewDraft, type PreviewResult } from './draftPreview'
import { ItemType } from './items'
import { THUMBNAIL_PATH, VIDEO_PATH } from './itemFiles'

const AUTO = 'data:image/png;base64,iVBORw0KGgo='
const PICKED = 'data:image/png;base64,cGlja2Vk'
const METRICS = { triangles: 708, materials: 3, meshes: 5, textures: 4, bodies: 5, entities: 1 }

function generating(overrides: Partial<PreviewDraft> = {}): PreviewDraft {
  return {
    id: 'draft-1',
    type: ItemType.WEARABLE,
    model: 'hat.glb',
    contents: { 'hat.glb': new Blob(['glb']) },
    category: 'hat',
    rarity: 'epic',
    isSmart: false,
    requiredPermissions: [],
    metrics: null,
    thumbnail: null,
    isAutoThumbnail: false,
    autoThumbnailCategory: null,
    validationIssues: [],
    ...overrides
  }
}

const rendered: PreviewResult = { metrics: METRICS, thumbnail: AUTO, thumbnailCategory: 'hat' }

describe('isThumbnailGenerating', () => {
  it('is true while a model has no thumbnail yet', () => {
    expect(isThumbnailGenerating(generating())).toBe(true)
  })

  it('is false for a thumbnail shipped in the zip, even while the metrics are still being measured', () => {
    expect(isThumbnailGenerating(generating({ thumbnail: PICKED }))).toBe(false)
  })

  it('is true again when a category change needs the auto thumbnail in another pose', () => {
    const draft = generating({
      thumbnail: AUTO,
      isAutoThumbnail: true,
      autoThumbnailCategory: 'hat',
      category: 'upper_body',
      metrics: METRICS
    })
    expect(isThumbnailGenerating(draft)).toBe(true)
  })
})

describe('applyPreviewResult', () => {
  it('gives the draft its metrics and the rendered thumbnail, also as its thumbnail file', () => {
    const draft = applyPreviewResult(generating(), rendered)
    expect(draft).toMatchObject({
      metrics: METRICS,
      thumbnail: AUTO,
      isAutoThumbnail: true,
      autoThumbnailCategory: 'hat'
    })
    expect(draft.contents[THUMBNAIL_PATH]).toBeInstanceOf(Blob)
  })

  it('keeps a thumbnail the creator picked while the preview was still rendering', () => {
    const picked = new Blob(['picked'])
    const draft = applyPreviewResult(
      generating({ thumbnail: PICKED, contents: { 'hat.glb': new Blob(['glb']), [THUMBNAIL_PATH]: picked } }),
      rendered
    )
    expect(draft).toMatchObject({ metrics: METRICS, thumbnail: PICKED, isAutoThumbnail: false })
    expect(draft.contents[THUMBNAIL_PATH]).toBe(picked)
  })

  it('keeps files added while the preview was rendering', () => {
    const video = new Blob(['mp4'])
    const draft = applyPreviewResult(
      generating({ contents: { 'hat.glb': new Blob(['glb']), [VIDEO_PATH]: video } }),
      rendered
    )
    expect(draft.contents[VIDEO_PATH]).toBe(video)
  })

  it('replaces its own auto thumbnail when the category change asked for a new pose', () => {
    const draft = applyPreviewResult(
      generating({ thumbnail: PICKED, isAutoThumbnail: true, autoThumbnailCategory: 'hat', category: 'upper_body' }),
      { ...rendered, thumbnailCategory: 'upper_body' }
    )
    expect(draft).toMatchObject({ thumbnail: AUTO, autoThumbnailCategory: 'upper_body' })
  })
})
