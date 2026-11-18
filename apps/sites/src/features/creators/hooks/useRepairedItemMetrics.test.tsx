import { beforeEach, describe, expect, it, vi } from 'vitest'
import { type ReactNode } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { renderHook, waitFor } from '@testing-library/react'
import { fetchContent } from '~/lib/builder'
import { ItemType, type Item } from '~/lib/items'
import { captureError } from '~/lib/monitoring'
import { useRepairedItemMetrics } from './useRepairedItemMetrics'

vi.mock('~/lib/builder', () => ({ fetchContent: vi.fn() }))
vi.mock('~/lib/monitoring', () => ({ captureError: vi.fn() }))

const MALE = 'urn:decentraland:off-chain:base-avatars:BaseMale'
const BOOT_SCENE = { triangles: 0, materials: 0, textures: 0, meshes: 1, bodies: 1, entities: 1 }
const model = {
  nodes: [{ mesh: 0 }],
  meshes: [{ primitives: [{ attributes: { POSITION: 0 } }] }],
  accessors: [{ count: 300 }],
  materials: [{}],
  textures: [{}]
}

function wearable(overrides: Partial<Item> = {}): Item {
  return {
    id: 'w1',
    name: 'Hat',
    description: '',
    thumbnail: 'thumbnail.png',
    owner: '0xabc',
    rarity: 'common',
    isPublished: false,
    isApproved: false,
    inCatalyst: false,
    type: ItemType.WEARABLE,
    data: { category: 'hat', tags: [], representations: [{ bodyShapes: [MALE], mainFile: 'hat.gltf', contents: [] }] },
    contents: { 'hat.gltf': 'bafmodel' },
    metrics: BOOT_SCENE,
    createdAt: 0,
    updatedAt: 0,
    ...overrides
  }
}

function render(item: Item) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  )
  return renderHook(() => useRepairedItemMetrics(item), { wrapper }).result
}

beforeEach(() => {
  vi.mocked(fetchContent).mockReset()
  vi.mocked(captureError).mockReset()
  vi.mocked(fetchContent).mockResolvedValue(new Blob([JSON.stringify(model)]))
})

describe('useRepairedItemMetrics', () => {
  it('counts the metrics of a wearable stored without geometry from its model', async () => {
    const result = render(wearable())
    await waitFor(() => expect(result.current).toMatchObject({ triangles: 100, materials: 1, textures: 1 }))
  })

  it('leaves alone an item whose metrics are fine, an emote, a texture wearable and one without its file', async () => {
    const image = wearable()
    image.data.representations[0].mainFile = 'skin.png'
    const untouched = [
      wearable({ metrics: { ...BOOT_SCENE, triangles: 10 } }),
      wearable({ type: ItemType.EMOTE }),
      image,
      wearable({ contents: {} })
    ]
    for (const item of untouched) expect(render(item).current).toBeNull()
    expect(fetchContent).not.toHaveBeenCalled()
  })

  it('reports a model that cannot be fetched and repairs nothing', async () => {
    vi.mocked(fetchContent).mockRejectedValue(new Error('storage down'))
    const result = render(wearable())
    await waitFor(() => expect(captureError).toHaveBeenCalled())
    expect(result.current).toBeNull()
  })
})
