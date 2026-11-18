import { beforeEach, describe, expect, it, vi } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { PreviewRenderer } from '@dcl/schemas'
import { type ReactNode } from 'react'

const getIsFeatureEnabled = vi.fn()
vi.mock('~/lib/featureFlags', async importOriginal => ({
  ...(await importOriginal<typeof import('~/lib/featureFlags')>()),
  getIsFeatureEnabled: () => getIsFeatureEnabled()
}))
const measureDownlinkMbps = vi.fn()
vi.mock('~/lib/connectionSpeed', () => ({ measureDownlinkMbps: () => measureDownlinkMbps() }))
const track = vi.fn()
vi.mock('~/lib/analytics', () => ({ track: (...args: unknown[]) => track(...args) }))

const { usePreviewRenderer } = await import('./usePreviewRenderer')

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>
}

beforeEach(() => {
  getIsFeatureEnabled.mockReset()
  measureDownlinkMbps.mockReset()
  track.mockReset()
})

describe('usePreviewRenderer', () => {
  it('waits for the flag and the probe, then settles on Unity over a good link', async () => {
    getIsFeatureEnabled.mockResolvedValue(true)
    measureDownlinkMbps.mockResolvedValue(48.26)
    const { result } = renderHook(() => usePreviewRenderer(), { wrapper })
    expect(result.current).toBeUndefined()
    await waitFor(() => expect(result.current).toBe(PreviewRenderer.UNITY))
    expect(track).toHaveBeenCalledWith('Preview Renderer Picked', {
      renderer: PreviewRenderer.UNITY,
      reason: 'connection-ok',
      measured_mbps: 48.3
    })
  })

  it('falls back to Babylon on a slow link', async () => {
    getIsFeatureEnabled.mockResolvedValue(true)
    measureDownlinkMbps.mockResolvedValue(3)
    const { result } = renderHook(() => usePreviewRenderer(), { wrapper })
    await waitFor(() => expect(result.current).toBe(PreviewRenderer.BABYLON))
    expect(track).toHaveBeenCalledWith(
      'Preview Renderer Picked',
      expect.objectContaining({ reason: 'slow-connection' })
    )
  })

  it('falls back to Babylon while the flag is off or unknown, without probing the link', async () => {
    getIsFeatureEnabled.mockResolvedValue(false)
    const { result } = renderHook(() => usePreviewRenderer(), { wrapper })
    await waitFor(() => expect(result.current).toBe(PreviewRenderer.BABYLON))
    expect(measureDownlinkMbps).not.toHaveBeenCalled()
    expect(track).toHaveBeenCalledWith('Preview Renderer Picked', expect.objectContaining({ reason: 'flag-off' }))
  })

  it("does not spend a data-saver user's data on a probe", async () => {
    getIsFeatureEnabled.mockResolvedValue(true)
    Object.defineProperty(navigator, 'connection', { value: { saveData: true }, configurable: true })
    try {
      const { result } = renderHook(() => usePreviewRenderer(), { wrapper })
      await waitFor(() => expect(result.current).toBe(PreviewRenderer.BABYLON))
      expect(measureDownlinkMbps).not.toHaveBeenCalled()
    } finally {
      Object.defineProperty(navigator, 'connection', { value: undefined, configurable: true })
    }
  })

  it('still mounts the preview when reporting the decision throws', async () => {
    getIsFeatureEnabled.mockResolvedValue(true)
    measureDownlinkMbps.mockResolvedValue(50)
    track.mockImplementation(() => {
      throw new Error('segment down')
    })
    const { result } = renderHook(() => usePreviewRenderer(), { wrapper })
    await waitFor(() => expect(result.current).toBe(PreviewRenderer.UNITY))
  })
})
