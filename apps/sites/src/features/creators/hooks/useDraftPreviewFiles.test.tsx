// @vitest-environment jsdom
import '@/data/lib/creators/test/setup'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import type { Session } from '@/data/lib/creators/auth'
import { ItemType, type Item } from '@/data/lib/creators/items'
import { useWallet } from '../store/wallet'

const api = vi.hoisted(() => ({ fetchContent: vi.fn() }))
vi.mock('@/data/lib/creators/builder', async importOriginal => ({ ...(await importOriginal<object>()), ...api }))

const { useDraftPreviewFiles } = await import('./useDraftPreviewFiles')

const CREATOR = '0x00000000000000000000000000000000000000c1'

function item(isPublished: boolean): Item {
  return {
    id: 'hat',
    name: 'Hat',
    thumbnail: 'thumbnail.png',
    isPublished,
    type: ItemType.WEARABLE,
    data: {
      category: 'hat',
      representations: [
        { bodyShapes: ['urn:decentraland:off-chain:base-avatars:BaseMale'], mainFile: 'male/hat.glb', contents: ['male/hat.glb'] },
        { bodyShapes: ['urn:decentraland:off-chain:base-avatars:BaseFemale'], mainFile: 'female/hat.glb', contents: ['female/hat.glb', 'female/tex.png'] }
      ]
    },
    contents: {
      'male/hat.glb': 'bafmale',
      'female/hat.glb': 'baffemale',
      'female/tex.png': 'baftex',
      'thumbnail.png': 'bafthumb',
      'video.mp4': 'bafvideo'
    }
  } as unknown as Item
}

let client: QueryClient
const wrapper = ({ children }: { children: ReactNode }) => (
  <QueryClientProvider client={client}>{children}</QueryClientProvider>
)

beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  api.fetchContent.mockReset().mockImplementation(async (hash: string) => new Blob([hash]))
  useWallet.setState({ session: { address: CREATOR } as unknown as Session, restored: true })
})

afterEach(() => {
  useWallet.setState({ session: null, restored: false })
})

describe('when the item is a draft', () => {
  it('should read the files its representations use, signed, keyed by path', async () => {
    const { result } = renderHook(() => useDraftPreviewFiles(item(false)), { wrapper })
    await waitFor(() => expect(result.current).toBeDefined())
    expect(Object.keys(result.current!).sort()).toEqual(['female/hat.glb', 'female/tex.png', 'male/hat.glb'])
    expect(await result.current!['female/tex.png'].text()).toBe('baftex')
    expect(api.fetchContent.mock.calls.map(([hash]) => hash).sort()).toEqual(['baffemale', 'bafmale', 'baftex'])
  })

  it('should give nothing when a file cannot be read', async () => {
    api.fetchContent.mockRejectedValueOnce(new Error('404'))
    const { result } = renderHook(() => useDraftPreviewFiles(item(false)), { wrapper })
    await waitFor(() => expect(api.fetchContent).toHaveBeenCalled())
    expect(result.current).toBeUndefined()
  })
})

describe('when the item is published', () => {
  it('should read nothing, as the preview loads its public files', () => {
    const { result } = renderHook(() => useDraftPreviewFiles(item(true)), { wrapper })
    expect(result.current).toBeUndefined()
    expect(api.fetchContent).not.toHaveBeenCalled()
  })
})
