import { beforeEach, describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { sendOverviewTrack as track } from '~/lib/overviewSegment'
import { OverviewSection } from '~/lib/overviewAnalytics'
import { AnimatedSection } from './AnimatedSection'

vi.mock('~/lib/overviewSegment', () => ({ sendOverviewTrack: vi.fn(), sendOverviewPage: vi.fn() }))

const viewport = vi.hoisted(() => ({ inView: false }))
vi.mock('react-intersection-observer', () => ({ useInView: () => ({ ref: vi.fn(), inView: viewport.inView }) }))

beforeEach(() => {
  viewport.inView = false
  vi.mocked(track).mockClear()
})

describe('AnimatedSection', () => {
  it('keeps the content hidden and unreported until it scrolls into view', () => {
    render(
      <AnimatedSection section={OverviewSection.WHY}>
        <span>child content</span>
      </AnimatedSection>
    )
    expect(screen.getByText('child content')).toBeInTheDocument()
    expect(screen.getByTestId('overview-section-Creators Why')).not.toHaveAttribute('data-visible')
    expect(track).not.toHaveBeenCalled()
  })

  it('reveals the content and reports the section once it is in view', () => {
    viewport.inView = true
    render(
      <AnimatedSection section={OverviewSection.WHY}>
        <span>child</span>
      </AnimatedSection>
    )
    expect(screen.getByTestId('overview-section-Creators Why')).toHaveAttribute('data-visible')
    expect(track).toHaveBeenCalledWith('Section Viewed', { section_viewed: 'Creators Why', mobile: false })
  })
})
