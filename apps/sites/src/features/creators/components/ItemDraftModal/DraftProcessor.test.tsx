import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, render } from '@testing-library/react'
import { PreviewRenderer } from '@dcl/schemas'
import { ItemType } from '~/lib/items'
import { type PreviewDraft } from '~/lib/draftPreview'

type PreviewProps = { id: string; onLoad?: () => void; onUpdate?: () => void }
const previewProps: PreviewProps[] = []
const getMetrics = vi.fn()
const controller = {
  scene: { getMetrics, getScreenshot: vi.fn(async () => 'data:image/png;base64,iVBORw0KGgo=') },
  emote: { getLength: vi.fn(async () => 0), goTo: vi.fn() }
}

vi.mock('~/components/WearablePreview', () => ({
  WearablePreview(props: PreviewProps) {
    previewProps.push(props)
    return <iframe id={props.id} title="preview" />
  },
  createWearablePreviewController: vi.fn(() => controller)
}))

vi.mock('~/hooks/usePreviewRenderer', () => ({ usePreviewRenderer: () => PreviewRenderer.UNITY }))

const { captureError } = vi.hoisted(() => ({ captureError: vi.fn() }))
vi.mock('~/lib/monitoring', () => ({ captureError }))

const { DraftProcessor, BOOT_BUDGET_MS, READ_BUDGET_MS, RUN_BUDGET_MS } = await import('./DraftProcessor')

const REAL_METRICS = { triangles: 708, materials: 3, meshes: 5, textures: 4, bodies: 5, entities: 1 }
const EMPTY_SCENE_METRICS = { triangles: 0, materials: 0, meshes: 0, textures: 0, bodies: 0, entities: 1 }
// Babylon's boot scene holds one empty mesh.
const BOOT_SCENE_METRICS = { triangles: 0, materials: 0, meshes: 1, textures: 0, bodies: 1, entities: 1 }

function wearableDraft(): PreviewDraft {
  const file = new File([new Blob(['glb'])], 'hat.glb')
  return {
    id: 'draft-1',
    type: ItemType.WEARABLE,
    model: 'hat.glb',
    contents: { 'hat.glb': file },
    category: null,
    rarity: 'epic',
    isSmart: false,
    requiredPermissions: [],
    metrics: null,
    thumbnail: null,
    isAutoThumbnail: false,
    autoThumbnailCategory: null,
    validationIssues: []
  }
}

/** Plays the iframe's side of the handshake: the UPDATE with the model goes out, then a LOAD comes back. */
async function iframeLoads() {
  const props = previewProps[previewProps.length - 1]
  await act(async () => {
    props.onUpdate?.()
    props.onLoad?.()
  })
}

async function elapse(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms)
  })
}

beforeEach(() => {
  vi.useFakeTimers()
  previewProps.length = 0
  getMetrics.mockReset()
  captureError.mockReset()
})
afterEach(() => {
  vi.useRealTimers()
})

describe('DraftProcessor', () => {
  it('fills the draft with the metrics the preview reports once it has loaded the model', async () => {
    getMetrics.mockResolvedValue(REAL_METRICS)
    const onResult = vi.fn()
    render(<DraftProcessor draft={wearableDraft()} flow="add_items" onResult={onResult} onError={vi.fn()} />)

    await iframeLoads()

    expect(onResult).toHaveBeenCalledTimes(1)
    expect(onResult.mock.calls[0][1]).toMatchObject({
      metrics: REAL_METRICS,
      thumbnail: expect.stringMatching(/^data:image\/png/)
    })
  })

  it('ignores a LOAD announced for the empty scene and reads the metrics off the render that follows', async () => {
    getMetrics.mockResolvedValueOnce(EMPTY_SCENE_METRICS).mockResolvedValue(REAL_METRICS)
    const onResult = vi.fn()
    const onError = vi.fn()
    render(<DraftProcessor draft={wearableDraft()} flow="add_items" onResult={onResult} onError={onError} />)

    await iframeLoads()
    expect(onResult).not.toHaveBeenCalled()

    await iframeLoads()
    expect(onError).not.toHaveBeenCalled()
    expect(onResult).toHaveBeenCalledTimes(1)
    expect(onResult.mock.calls[0][1].metrics).toEqual(REAL_METRICS)
  })

  it('ignores a reading of the boot scene that counts one empty mesh', async () => {
    getMetrics.mockResolvedValueOnce(BOOT_SCENE_METRICS).mockResolvedValue(REAL_METRICS)
    const onResult = vi.fn()
    render(<DraftProcessor draft={wearableDraft()} flow="add_items" onResult={onResult} onError={vi.fn()} />)

    await iframeLoads()
    expect(onResult).not.toHaveBeenCalled()

    await iframeLoads()
    expect(onResult).toHaveBeenCalledTimes(1)
    expect(onResult.mock.calls[0][1].metrics).toEqual(REAL_METRICS)
  })

  it('treats the Unity boot scene having no item as the empty scene, not as a failed attempt', async () => {
    getMetrics.mockRejectedValueOnce(new Error('No requested item is loaded')).mockResolvedValue(REAL_METRICS)
    const onResult = vi.fn()
    const onError = vi.fn()
    render(<DraftProcessor draft={wearableDraft()} flow="add_items" onResult={onResult} onError={onError} />)

    await iframeLoads()
    await elapse(10_000)
    expect(onResult).not.toHaveBeenCalled()
    expect(onError).not.toHaveBeenCalled()

    await iframeLoads()
    expect(onResult.mock.calls[0][1].metrics).toEqual(REAL_METRICS)
  })

  it('does not give up on a model that takes a while to show up in the preview', async () => {
    getMetrics.mockResolvedValueOnce(EMPTY_SCENE_METRICS).mockResolvedValue(REAL_METRICS)
    const onResult = vi.fn()
    const onError = vi.fn()
    render(<DraftProcessor draft={wearableDraft()} flow="add_items" onResult={onResult} onError={onError} />)

    await iframeLoads()
    await elapse(10_000)
    expect(onError).not.toHaveBeenCalled()

    await iframeLoads()
    expect(onResult.mock.calls[0][1].metrics).toEqual(REAL_METRICS)
  })

  it('fails the draft once when the preview keeps erroring, and ignores anything after that', async () => {
    getMetrics.mockRejectedValue(new Error('The scene was disposed, a newer render has replaced it'))
    const onResult = vi.fn()
    const onError = vi.fn()
    render(<DraftProcessor draft={wearableDraft()} flow="add_items" onResult={onResult} onError={onError} />)

    await iframeLoads()
    await elapse(5000)
    expect(onError).toHaveBeenCalledTimes(1)

    getMetrics.mockResolvedValue(REAL_METRICS)
    await iframeLoads()
    expect(onResult).not.toHaveBeenCalled()
    expect(onError).toHaveBeenCalledTimes(1)
  })

  it('reboots a preview that never loads the model and processes the draft on the fresh one', async () => {
    getMetrics.mockResolvedValue(REAL_METRICS)
    const onResult = vi.fn()
    const onError = vi.fn()
    render(<DraftProcessor draft={wearableDraft()} flow="add_items" onResult={onResult} onError={onError} />)
    const wedged = document.querySelector('iframe')

    await elapse(BOOT_BUDGET_MS)
    expect(document.querySelector('iframe')).not.toBe(wedged)

    await iframeLoads()
    expect(onResult).toHaveBeenCalledTimes(1)
    expect(onError).not.toHaveBeenCalled()
  })

  it('fails the draft as timed out when the fresh preview does not load it either, so the queue moves on', async () => {
    const onResult = vi.fn()
    const onError = vi.fn()
    render(<DraftProcessor draft={wearableDraft()} flow="add_items" onResult={onResult} onError={onError} />)

    await elapse(BOOT_BUDGET_MS)
    const retried = document.querySelector('iframe')
    await elapse(BOOT_BUDGET_MS)

    expect(onError).toHaveBeenCalledWith('draft-1', 'timeout')
    // The next file starts on a fresh iframe instead of the wedged one.
    expect(document.querySelector('iframe')).not.toBe(retried)
    expect(captureError).toHaveBeenCalledWith(
      expect.any(Error),
      expect.objectContaining({ flow: 'add_items', step: 'preview_timeout' })
    )
    expect(onResult).not.toHaveBeenCalled()
  })

  it('ignores an answer the wedged preview sends after it was replaced', async () => {
    let answerLate: (metrics: typeof REAL_METRICS) => void = () => {}
    getMetrics
      .mockImplementationOnce(() => new Promise(resolve => (answerLate = resolve)))
      .mockResolvedValue(REAL_METRICS)
    const onResult = vi.fn()
    render(<DraftProcessor draft={wearableDraft()} flow="add_items" onResult={onResult} onError={vi.fn()} />)

    await iframeLoads()
    await elapse(READ_BUDGET_MS)
    await act(async () => answerLate(REAL_METRICS))
    expect(onResult).not.toHaveBeenCalled()

    await iframeLoads()
    expect(onResult).toHaveBeenCalledTimes(1)
  })

  it('gives a model that has loaded time to be read, beyond the budget for a scene that never loads', async () => {
    getMetrics.mockImplementation(
      () => new Promise(resolve => setTimeout(() => resolve(REAL_METRICS), RUN_BUDGET_MS + 10_000))
    )
    const onResult = vi.fn()
    const onError = vi.fn()
    render(<DraftProcessor draft={wearableDraft()} flow="add_items" onResult={onResult} onError={onError} />)
    const iframe = document.querySelector('iframe')

    await iframeLoads()
    await elapse(RUN_BUDGET_MS + 10_000)

    expect(onResult).toHaveBeenCalledTimes(1)
    expect(onError).not.toHaveBeenCalled()
    expect(document.querySelector('iframe')).toBe(iframe)
  })

  it('reads the model again when its LOAD lands while the empty scene is still being read', async () => {
    let answerEmpty: (metrics: typeof EMPTY_SCENE_METRICS) => void = () => {}
    getMetrics
      .mockImplementationOnce(() => new Promise(resolve => (answerEmpty = resolve)))
      .mockResolvedValue(REAL_METRICS)
    const onResult = vi.fn()
    render(<DraftProcessor draft={wearableDraft()} flow="add_items" onResult={onResult} onError={vi.fn()} />)

    await iframeLoads()
    await act(async () => previewProps[previewProps.length - 1].onLoad?.())
    await act(async () => answerEmpty(EMPTY_SCENE_METRICS))

    expect(onResult).toHaveBeenCalledTimes(1)
    expect(onResult.mock.calls[0][1].metrics).toEqual(REAL_METRICS)
  })
})
