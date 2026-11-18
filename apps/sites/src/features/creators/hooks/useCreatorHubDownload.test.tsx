import type { ReactNode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { renderHook, waitFor } from '@testing-library/react'
import { useCreatorHubDownload } from './useCreatorHubDownload'

const WINDOWS_UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)'
const RELEASE = 'https://github.com/decentraland/creator-hub/releases/download/0.50.0'

function renderDownload(userAgent: string, response: Partial<Response>) {
  vi.spyOn(navigator, 'userAgent', 'get').mockReturnValue(userAgent)
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue(response))
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  )
  return renderHook(() => useCreatorHubDownload(true), { wrapper })
}

const release = (names: string[]) => ({
  ok: true,
  status: 200,
  json: () => Promise.resolve({ assets: names.map(name => ({ name, browser_download_url: `${RELEASE}/${name}` })) })
})

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

describe('useCreatorHubDownload', () => {
  it('is loading until the release resolves, then offers the installer', async () => {
    const { result } = renderDownload(WINDOWS_UA, release(['ch-win-x64.exe']))
    expect(result.current).toEqual({ fallback: 'loading' })
    await waitFor(() => expect(result.current.download?.href).toBe(`${RELEASE}/ch-win-x64.exe`))
  })

  it('tells an OS without an installer apart from a release that could not be read', async () => {
    const linux = renderDownload('Mozilla/5.0 (X11; Linux x86_64)', release(['ch-win-x64.exe']))
    await waitFor(() => expect(linux.result.current).toEqual({ fallback: 'unsupported' }))

    const rateLimited = renderDownload(WINDOWS_UA, { ok: false, status: 403, body: null })
    await waitFor(() => expect(rateLimited.result.current).toEqual({ fallback: 'unavailable' }))
  })
})
