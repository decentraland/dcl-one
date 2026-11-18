import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { TranslationProvider } from '~/intl'
import { sendOverviewTrack as track } from '~/lib/overviewSegment'
import { Connect } from './Connect'

vi.mock('~/lib/overviewSegment', () => ({ sendOverviewTrack: vi.fn(), sendOverviewPage: vi.fn() }))
vi.mock('react-intersection-observer', () => ({ useInView: () => ({ ref: vi.fn(), inView: true }) }))

beforeEach(() => vi.mocked(track).mockClear())

const renderSection = () =>
  render(
    <TranslationProvider>
      <Connect />
    </TranslationProvider>
  )

describe('Connect', () => {
  it('quotes each creator with a link to their profile and tracks the click', () => {
    renderSection()
    const cards = screen.getAllByTestId('overview-testimonial')
    // Three copies of every testimonial: the carousel clones its edges to loop.
    expect(new Set(cards.map(card => card.getAttribute('href'))).size).toBe(5)
    expect(cards[0]).toHaveAttribute('href', 'https://x.com/MrDhingia')
    expect(cards[0]).toHaveTextContent('MrDhingia')

    fireEvent.click(cards[0])
    expect(track).toHaveBeenCalledWith('Click', { place: 'Creators Connect', title: 'MrDhingia' })
  })

  it('invites to the Discord and tracks it', () => {
    renderSection()
    const discord = screen.getByTestId('overview-discord')
    expect(discord).toHaveAttribute('href', 'https://dcl.gg/discord')
    expect(discord).toHaveTextContent('Join the Community')

    fireEvent.click(discord)
    expect(track).toHaveBeenCalledWith('Click', { place: 'Creators Connect', title: 'join-discord' })
  })
})
