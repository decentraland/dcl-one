import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook } from '@testing-library/react'

const { showToast } = vi.hoisted(() => ({ showToast: vi.fn() }))
vi.mock('../../design', () => ({ showToast }))

const { NEARBY_VOICE_BLOCKED, useNearbyVoiceRefusals } = await import('../../features/voice/NearbyVoiceRefusals')

beforeEach(() => showToast.mockClear())

describe('nearby voice refusals', () => {
  it('a refusal counted before mount is not toasted; each later one is', () => {
    const { rerender } = renderHook(({ refusals }) => useNearbyVoiceRefusals(refusals), { initialProps: { refusals: 3 } })
    expect(showToast).not.toHaveBeenCalled()
    rerender({ refusals: 4 })
    expect(showToast).toHaveBeenCalledTimes(1)
    expect(showToast).toHaveBeenLastCalledWith(NEARBY_VOICE_BLOCKED)
    rerender({ refusals: 5 })
    expect(showToast).toHaveBeenCalledTimes(2)
  })

  it('an unchanged count is not toasted again', () => {
    const { rerender } = renderHook(({ refusals }) => useNearbyVoiceRefusals(refusals), { initialProps: { refusals: 0 } })
    rerender({ refusals: 0 })
    expect(showToast).not.toHaveBeenCalled()
  })
})
