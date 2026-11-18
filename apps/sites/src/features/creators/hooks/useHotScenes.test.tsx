import type { ReactNode } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { QueryClientProvider } from '@tanstack/react-query'
import { renderHook, waitFor } from '@testing-library/react'
import { setErrorForwarder } from '~/lib/monitoring'
import { createQueryClient } from '~/lib/queryClient'
import { useHotScenes } from './useHotScenes'

const reported = vi.fn()

function renderFeed() {
  const client = createQueryClient()
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  )
  return renderHook(() => useHotScenes(), { wrapper })
}

beforeEach(() => {
  vi.spyOn(console, 'error').mockImplementation(() => undefined)
  setErrorForwarder(reported)
})

afterEach(() => {
  setErrorForwarder(null)
  reported.mockReset()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

describe('useHotScenes', () => {
  it('retries a failing feed once, then reports the server error once with its status', async () => {
    const fetchMock = vi.fn().mockResolvedValue({ ok: false, status: 502, body: null })
    vi.stubGlobal('fetch', fetchMock)

    const { result } = renderFeed()

    await waitFor(() => expect(result.current.isError).toBe(true), { timeout: 3000 })
    expect(fetchMock).toHaveBeenCalledTimes(2)
    expect(reported).toHaveBeenCalledTimes(1)
    expect(reported).toHaveBeenCalledWith(expect.anything(), expect.objectContaining({ http_status: 502 }))
  })

  it('does not report a feed the visitor’s network never reached', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('Failed to fetch')))

    const { result } = renderFeed()

    await waitFor(() => expect(result.current.isError).toBe(true), { timeout: 3000 })
    expect(reported).not.toHaveBeenCalled()
  })
})
