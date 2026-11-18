import { beforeEach, describe, expect, it, vi } from 'vitest'
import { renderHook } from '@testing-library/react'
import { sendOverviewTrack as track } from '~/lib/overviewSegment'
import { OverviewSection } from '~/lib/overviewAnalytics'
import { useSectionViewed } from './useSectionViewed'

vi.mock('~/lib/overviewSegment', () => ({ sendOverviewTrack: vi.fn(), sendOverviewPage: vi.fn() }))

// A stand-in viewport that evaluates the `max-width` query it is asked about.
const viewport = vi.hoisted(() => ({ width: 1280 }))
vi.mock('~/hooks/useMediaQuery', () => ({
  useMediaQuery: (query: string) => viewport.width <= Number(/max-width:\s*([\d.]+)px/.exec(query)?.[1] ?? 0)
}))

beforeEach(() => {
  vi.mocked(track).mockClear()
  viewport.width = 1280
})

describe('useSectionViewed', () => {
  it('reports the section once, the first time it comes into view', () => {
    const { rerender } = renderHook(({ inView }) => useSectionViewed(OverviewSection.WHY, inView), {
      initialProps: { inView: false }
    })
    expect(track).not.toHaveBeenCalled()

    rerender({ inView: true })
    rerender({ inView: true })
    rerender({ inView: false })
    rerender({ inView: true })

    expect(track).toHaveBeenCalledTimes(1)
    expect(track).toHaveBeenCalledWith('Section Viewed', { section_viewed: 'Creators Why', mobile: false })
  })

  it('flags phone viewports as mobile with sites’ cut: 767px is mobile, 768px is not', () => {
    viewport.width = 767
    renderHook(() => useSectionViewed(OverviewSection.FAQS, true))
    expect(track).toHaveBeenLastCalledWith('Section Viewed', { section_viewed: 'Creators Faqs', mobile: true })

    viewport.width = 768
    renderHook(() => useSectionViewed(OverviewSection.LEARN, true))
    expect(track).toHaveBeenLastCalledWith('Section Viewed', { section_viewed: 'Creators Learn', mobile: false })
  })
})
