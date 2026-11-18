import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { TranslationProvider } from '~/intl'
import { sendOverviewTrack as track } from '~/lib/overviewSegment'
import type { HotScene } from '~/lib/hotScenes'
import { LiveScenes } from './LiveScenes'

vi.mock('~/lib/overviewSegment', () => ({ sendOverviewTrack: vi.fn(), sendOverviewPage: vi.fn() }))
vi.mock('react-intersection-observer', () => ({ useInView: () => ({ ref: vi.fn(), inView: true }) }))

const feed = vi.hoisted(() => ({ data: undefined as HotScene[] | undefined }))
vi.mock('~/hooks/useHotScenes', () => ({ useHotScenes: () => ({ data: feed.data }) }))

const aScene = (overrides: Partial<HotScene>): HotScene => ({
  id: 'scene-1',
  name: 'A Scene',
  baseCoords: [10, 20],
  usersTotalCount: 5,
  parcels: [[10, 20]],
  thumbnail: 'https://img.example.com/scene.png',
  ...overrides
})

beforeEach(() => {
  feed.data = undefined
  vi.mocked(track).mockClear()
})

const renderSection = () =>
  render(
    <TranslationProvider>
      <LiveScenes />
    </TranslationProvider>
  )

describe('LiveScenes', () => {
  it('renders nothing while the feed loads or when no scene is occupied', () => {
    const { container, rerender } = renderSection()
    expect(container).toBeEmptyDOMElement()

    feed.data = [aScene({ name: 'Genesis Plaza', usersTotalCount: 80 }), aScene({ id: 'e', usersTotalCount: 0 })]
    rerender(
      <TranslationProvider>
        <LiveScenes />
      </TranslationProvider>
    )
    expect(container).toBeEmptyDOMElement()
  })

  it('lists the busiest scenes first, linking each to its place page and all to the places site', () => {
    feed.data = [
      aScene({ id: 'quiet', name: 'Quiet Scene', usersTotalCount: 2, baseCoords: [1, 1] }),
      aScene({ id: 'busy', name: 'Busy Scene', usersTotalCount: 40, baseCoords: [2, -2] }),
      aScene({ id: 'plaza', name: 'Genesis Plaza', usersTotalCount: 80 })
    ]
    renderSection()
    const cards = screen.getAllByTestId('overview-scene-card')
    expect(cards.map(card => card.getAttribute('href'))).toEqual([
      'https://decentraland.zone/places/place/2,-2',
      'https://decentraland.zone/places/place/1,1'
    ])
    expect(cards[0]).toHaveTextContent('40 online')
    expect(screen.queryByText('Genesis Plaza')).toBeNull()
    expect(screen.getByTestId('overview-live-scenes-all')).toHaveAttribute('href', 'https://decentraland.zone/places')
    expect(track).toHaveBeenCalledWith('Section Viewed', { section_viewed: 'Creators Live Scenes', mobile: false })

    fireEvent.click(cards[0])
    expect(track).toHaveBeenCalledWith('Click', { place: 'Creators Live Scenes', title: 'Busy Scene' })
  })
})
