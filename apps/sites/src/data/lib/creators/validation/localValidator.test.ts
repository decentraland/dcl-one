import { beforeEach, describe, expect, it, vi } from 'vitest'
import { BodyShape } from '@dcl/schemas'
import { englishMessage } from '~/intl'
import { captureError } from '../monitoring'
import { ItemType, type Item } from '../items'

const validate = vi.fn()
vi.mock('../monitoring', () => ({ captureError: vi.fn() }))
vi.mock('@dcl-regenesislabs/wearable-validator', () => ({
  validate: (...args: unknown[]) => validate(...args),
  checks: {}
}))

const { getValidator, setValidator } = await import('./index')
const { localValidator } = await import('./localValidator')

const item: Item = {
  id: 'w1',
  name: 'Hat',
  description: '',
  thumbnail: 'thumbnail.png',
  owner: '0xabc',
  isPublished: false,
  isApproved: false,
  inCatalyst: false,
  type: ItemType.WEARABLE,
  data: {
    category: 'hat',
    hides: ['hair'],
    representations: [
      { bodyShapes: [BodyShape.MALE], mainFile: 'male/hat.glb', contents: ['male/hat.glb'] },
      { bodyShapes: [BodyShape.FEMALE], mainFile: 'female/hat.glb', contents: ['female/hat.glb'] }
    ]
  },
  contents: { 'male/hat.glb': 'bafyM', 'female/hat.glb': 'bafyF', 'thumbnail.png': 'bafyThumb' },
  createdAt: 1,
  updatedAt: 1
}

const single: Item = {
  ...item,
  data: { ...item.data, representations: [item.data.representations[0]] }
}

const finding = { check: 'triangle-count', severity: 'error', message: 'Too many triangles', where: 'female/hat.glb' }
const fetchMock = vi.fn()

beforeEach(() => {
  validate.mockReset().mockResolvedValue({ findings: [finding], checks: [] })
  fetchMock.mockReset().mockImplementation(async () => new Response(new Uint8Array([1, 2, 3])))
  vi.stubGlobal('fetch', fetchMock)
  setValidator(localValidator)
})

describe('local validator', () => {
  it('checks each body shape model of a saved item and tags its issues with that shape', async () => {
    validate.mockImplementation(async ({ files }: { files: Map<string, Uint8Array> }) =>
      files.has('female/hat.glb') ? { findings: [finding], checks: [] } : { findings: [], checks: [] }
    )
    const result = await getValidator().validate(
      { kind: 'item', item },
      { type: ItemType.WEARABLE, category: 'hat', hides: ['hair'] }
    )
    expect(fetchMock.mock.calls.map(([url]) => url)).toEqual([
      'https://builder-api.decentraland.zone/v1/storage/contents/bafyM',
      'https://builder-api.decentraland.zone/v1/storage/contents/bafyF'
    ])
    const female = validate.mock.calls.find(([input]) => input.files.has('female/hat.glb'))!
    expect(female[0].metadata).toEqual({
      data: {
        category: 'hat',
        hides: ['hair'],
        representations: [{ bodyShapes: [BodyShape.FEMALE], mainFile: 'female/hat.glb', contents: ['female/hat.glb'] }]
      }
    })
    expect(female[1].groups).toEqual(['model', 'emote'])
    expect(result.issues).toEqual([
      {
        code: 'triangle-count',
        severity: 'error',
        message: 'Too many triangles',
        where: 'female/hat.glb',
        bodyShapes: [BodyShape.FEMALE]
      }
    ])
  })

  it('checks a model shared by both body shapes once, without tagging its issues', async () => {
    const unisex = {
      ...item,
      contents: { 'male/hat.glb': 'bafySame', 'female/hat.glb': 'bafySame', 'thumbnail.png': 'bafyThumb' }
    }
    const result = await getValidator().validate({ kind: 'item', item: unisex }, { type: ItemType.WEARABLE })
    expect(validate).toHaveBeenCalledTimes(1)
    expect(validate.mock.calls[0][0].metadata.data.representations[0].bodyShapes).toEqual([
      BodyShape.MALE,
      BodyShape.FEMALE
    ])
    expect(result.issues[0].bodyShapes).toBeUndefined()
  })

  it('checks every model of an unsaved file update', async () => {
    await getValidator().validate(
      {
        kind: 'blob',
        contents: { 'male/a.glb': new Blob(['m']), 'female/b.glb': new Blob(['f']) },
        mainFile: 'male/a.glb',
        representations: [
          { mainFile: 'male/a.glb', bodyShapes: [BodyShape.MALE] },
          { mainFile: 'female/b.glb', bodyShapes: [BodyShape.FEMALE] }
        ]
      },
      { type: ItemType.WEARABLE }
    )
    expect(validate.mock.calls.map(([input]) => [...input.files.keys()])).toEqual([['male/a.glb'], ['female/b.glb']])
  })

  it('hands an emote its audio, its loop setting and names the model explicitly, whatever its extension case', async () => {
    await getValidator().validate(
      {
        kind: 'blob',
        contents: { 'Dance.GLB': new Blob(['x']), 'sound.wav': new Blob(['a']), 'thumbnail.png': new Blob(['t']) },
        mainFile: 'Dance.GLB'
      },
      { type: ItemType.EMOTE, loop: true }
    )
    expect(fetchMock).not.toHaveBeenCalled()
    const [input] = validate.mock.calls[0]
    expect([...input.files.keys()]).toEqual(['Dance.GLB', 'sound.wav'])
    expect(input.metadata.emoteDataADR74.loop).toBe(true)
    expect(input.metadata.emoteDataADR74.representations[0]).toMatchObject({ mainFile: 'Dance.GLB' })
  })

  it('loads a unisex emote once with its own audio, so the audio counts once', async () => {
    const emote = {
      ...item,
      type: ItemType.EMOTE,
      data: {
        representations: [
          { bodyShapes: [BodyShape.MALE], mainFile: 'male/dance.glb', contents: ['male/dance.glb', 'male/sound.mp3'] },
          {
            bodyShapes: [BodyShape.FEMALE],
            mainFile: 'female/dance.glb',
            contents: ['female/dance.glb', 'female/sound.mp3']
          }
        ]
      },
      contents: { 'male/dance.glb': 'a', 'male/sound.mp3': 'b', 'female/dance.glb': 'a', 'female/sound.mp3': 'b' }
    } as Item
    fetchMock.mockImplementation(async () => new Response(new Uint8Array([1])))
    await getValidator().validate({ kind: 'item', item: emote }, { type: ItemType.EMOTE })
    expect(validate).toHaveBeenCalledTimes(1)
    expect([...validate.mock.calls[0][0].files.keys()]).toEqual(['male/dance.glb', 'male/sound.mp3'])
  })

  it('shows a crashed check even when other checks found something', async () => {
    validate.mockResolvedValue({
      findings: [{ ...finding, severity: 'warning' }],
      checks: [{ check: 'skeleton', status: 'errored', skipReason: 'boom' }]
    })
    const result = await getValidator().validate({ kind: 'item', item: single }, { type: ItemType.WEARABLE })
    expect(result.issues.map(issue => issue.message)).toEqual([
      'Too many triangles',
      englishMessage('item_editor.validation.check_crashed')
    ])
  })

  it('reports a failed download without leaving the body open', async () => {
    const cancel = vi.fn().mockResolvedValue(undefined)
    fetchMock.mockResolvedValue({ ok: false, status: 404, body: { cancel } })
    await expect(getValidator().validate({ kind: 'item', item: single }, { type: ItemType.WEARABLE })).rejects.toThrow(
      /404/
    )
    expect(cancel).toHaveBeenCalled()
  })

  it('reports a crashed check as a warning rather than a pass, without blocking on it', async () => {
    validate.mockResolvedValue({ findings: [], checks: [{ check: 'skeleton', status: 'errored', skipReason: 'boom' }] })
    const result = await getValidator().validate({ kind: 'item', item: single }, { type: ItemType.WEARABLE })
    expect(result.issues).toEqual([
      { code: 'file-format', severity: 'warning', message: englishMessage('item_editor.validation.check_crashed') }
    ])
    // The raw exception goes to Sentry, never to the creator.
    expect(captureError).toHaveBeenCalled()
  })

  it('hides category-unknown warnings and reports an unparseable model as an error', async () => {
    validate.mockResolvedValue({
      findings: [{ ...finding, severity: 'warning', data: { reason: 'category-unknown' } }],
      checks: [{ check: 'skeleton', status: 'skipped', skipReason: '"hat.glb" failed to parse' }]
    })
    const result = await getValidator().validate({ kind: 'item', item: single }, { type: ItemType.WEARABLE })
    expect(result.issues).toEqual([{ code: 'file-format', severity: 'error', message: '"hat.glb" failed to parse' }])
  })

  it('skips texture-only wearables', async () => {
    const png = {
      ...item,
      data: { ...item.data, representations: [{ bodyShapes: [BodyShape.MALE], mainFile: 'eyes.png', contents: [] }] },
      contents: { 'eyes.png': 'bafyE' }
    } as Item
    await expect(getValidator().validate({ kind: 'item', item: png }, { type: ItemType.WEARABLE })).resolves.toEqual({
      issues: []
    })
    expect(validate).not.toHaveBeenCalled()
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('aborts a run whose signal was cancelled', async () => {
    const controller = new AbortController()
    controller.abort()
    await expect(
      getValidator().validate(
        { kind: 'item', item: single },
        { type: ItemType.WEARABLE },
        { signal: controller.signal }
      )
    ).rejects.toThrow()
  })
})

describe('thumbnail validation', () => {
  const thumbnailFinding = (message: string, extra: Record<string, unknown> = {}) => ({
    check: 'thumbnail',
    severity: 'warning',
    message,
    where: 'thumbnail.png',
    ...extra
  })

  it('reports the rule book thumbnail findings', async () => {
    validate.mockResolvedValue({
      findings: [thumbnailFinding('Thumbnail background does not look transparent — remove the background.')],
      checks: []
    })
    const result = await getValidator().validateThumbnail({ kind: 'blob', blob: new Blob(['png']) })
    const [input, options] = validate.mock.calls[0]
    expect([...input.files.keys()]).toEqual(['thumbnail.png'])
    expect(options.checks).toEqual(['thumbnail'])
    expect(result.issues).toEqual([
      {
        code: 'thumbnail',
        severity: 'warning',
        message: 'Thumbnail background does not look transparent — remove the background.',
        where: 'thumbnail.png'
      }
    ])
  })

  it('loads a saved item thumbnail from storage', async () => {
    validate.mockResolvedValue({ findings: [], checks: [] })
    const result = await getValidator().validateThumbnail({ kind: 'item', item })
    expect(fetchMock.mock.calls[0][0]).toBe('https://builder-api.decentraland.zone/v1/storage/contents/bafyThumb')
    expect(result.issues).toEqual([])
  })
})
