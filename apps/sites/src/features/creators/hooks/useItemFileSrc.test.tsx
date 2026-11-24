// @vitest-environment jsdom
import '@/data/lib/creators/test/setup'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import type { Session } from '@/data/lib/creators/auth'
import { privateKey } from '@/data/lib/creators/queryKeys'
import { useWallet } from '../store/wallet'

const api = vi.hoisted(() => ({ fetchContent: vi.fn() }))
vi.mock('@/data/lib/creators/builder', async importOriginal => ({ ...(await importOriginal<object>()), ...api }))

const { useItemFileSrc } = await import('./useItemFileSrc')

const CREATOR = '0x00000000000000000000000000000000000000c1'
const draft = { isPublished: false }
const published = { isPublished: true }

let client: QueryClient
let made: string[]
let revoked: string[]

function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>
}

beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  made = []
  revoked = []
  api.fetchContent.mockReset().mockImplementation(async (hash: string) => new Blob([hash]))
  vi.spyOn(URL, 'createObjectURL').mockImplementation(() => {
    const url = `blob:test/${made.length}`
    made.push(url)
    return url
  })
  vi.spyOn(URL, 'revokeObjectURL').mockImplementation(url => {
    revoked.push(url)
  })
  useWallet.setState({ session: { address: CREATOR } as unknown as Session, restored: true })
})

afterEach(() => {
  vi.restoreAllMocks()
  useWallet.setState({ session: null, restored: false })
})

describe('when the item is published', () => {
  it('should show the file from its plain storage URL without reading it', () => {
    const { result } = renderHook(() => useItemFileSrc(published, 'bafthumb'), { wrapper })
    expect(result.current).toBe('http://localhost:3000/builder-api/v1/storage/contents/bafthumb')
    expect(api.fetchContent).not.toHaveBeenCalled()
  })
})

describe('when the item is a draft', () => {
  it("should show the file read by the creator's session through an object URL", async () => {
    const { result } = renderHook(() => useItemFileSrc(draft, 'bafthumb'), { wrapper })
    await waitFor(() => expect(result.current).toBe('blob:test/0'))
    expect(api.fetchContent).toHaveBeenCalledWith('bafthumb', expect.any(AbortSignal))
  })

  it("should keep the file under the creator's account", async () => {
    const { result } = renderHook(() => useItemFileSrc(draft, 'bafthumb'), { wrapper })
    await waitFor(() => expect(result.current).not.toBeNull())
    expect(client.getQueryData(privateKey(CREATOR, 'item-file', 'bafthumb'))).toBeInstanceOf(Blob)
  })

  it('should revoke the object URL once the file is no longer shown', async () => {
    const { result, unmount } = renderHook(() => useItemFileSrc(draft, 'bafthumb'), { wrapper })
    await waitFor(() => expect(result.current).toBe('blob:test/0'))
    unmount()
    expect(revoked).toEqual(['blob:test/0'])
  })

  it('should revoke the previous object URL when the file changes', async () => {
    const { result, rerender } = renderHook(({ hash }) => useItemFileSrc(draft, hash), {
      wrapper,
      initialProps: { hash: 'bafone' }
    })
    await waitFor(() => expect(result.current).toBe('blob:test/0'))
    rerender({ hash: 'baftwo' })
    await waitFor(() => expect(result.current).toBe('blob:test/1'))
    expect(revoked).toEqual(['blob:test/0'])
  })

  it('should show nothing when the file cannot be read', async () => {
    api.fetchContent.mockRejectedValue(new Error('404'))
    const { result } = renderHook(() => useItemFileSrc(draft, 'bafthumb'), { wrapper })
    await waitFor(() => expect(api.fetchContent).toHaveBeenCalled())
    expect(result.current).toBeNull()
  })

  it('should read nothing while nobody is signed in', () => {
    useWallet.setState({ session: null })
    const { result } = renderHook(() => useItemFileSrc(draft, 'bafthumb'), { wrapper })
    expect(result.current).toBeNull()
    expect(api.fetchContent).not.toHaveBeenCalled()
  })
})

describe('when many drafts show at once', () => {
  it('should read at most four files at a time', async () => {
    const pending: Array<() => void> = []
    let inFlight = 0
    let most = 0
    api.fetchContent.mockImplementation(
      (hash: string) =>
        new Promise<Blob>(resolve => {
          inFlight++
          most = Math.max(most, inFlight)
          pending.push(() => {
            inFlight--
            resolve(new Blob([hash]))
          })
        })
    )
    const hashes = Array.from({ length: 10 }, (_, i) => `baf${i}`)
    const { result } = renderHook(() => hashes.map(hash => useItemFileSrc(draft, hash)), { wrapper })
    await waitFor(() => expect(pending.length).toBe(4))
    while (pending.length) {
      pending.shift()!()
      await waitFor(() => expect(inFlight).toBeLessThanOrEqual(4))
      await new Promise(resolve => setTimeout(resolve, 0))
    }
    await waitFor(() => expect(result.current.every(src => src?.startsWith('blob:'))).toBe(true))
    expect(most).toBe(4)
  })
})

describe('when there is no file', () => {
  it('should show nothing', () => {
    const { result } = renderHook(() => useItemFileSrc(draft, undefined), { wrapper })
    expect(result.current).toBeNull()
  })
})
