import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, within } from '@testing-library/react'
import { TranslationProvider } from '~/intl'
import { useLocale } from '~/store/locale'
import { sendOverviewTrack as track } from '~/lib/overviewSegment'
import { CreateCards } from './CreateCards'

vi.mock('~/lib/overviewSegment', () => ({ sendOverviewTrack: vi.fn(), sendOverviewPage: vi.fn() }))
vi.mock('react-intersection-observer', () => ({ useInView: () => ({ ref: vi.fn(), inView: true }) }))

const viewport = vi.hoisted(() => ({ mobile: false }))
vi.mock('~/hooks/useMediaQuery', () => ({ useMediaQuery: () => viewport.mobile }))

beforeEach(() => {
  viewport.mobile = false
  useLocale.setState({ locale: 'en' })
  vi.mocked(track).mockClear()
})

const renderSection = () =>
  render(
    <TranslationProvider>
      <CreateCards />
    </TranslationProvider>
  )

// The carousel renders every card three times (clones for the infinite loop) and exposes only the active
// slide to role queries, so cards are found by test id and the visible one by its slide's active flag.
const cards = () => screen.getAllByTestId('overview-create-card')
const activeCard = () =>
  cards().find(card => card.closest('[data-testid="carousel-slide"]')?.hasAttribute('data-active'))!
const linkNamed = (label: string) =>
  screen.getAllByTestId('overview-create-link').find(link => link.textContent === label)

const downloadLinks = () =>
  screen
    .getAllByTestId('overview-create-link')
    .filter(link => link.getAttribute('href') === 'https://decentraland.zone/download/creator-hub')

describe('CreateCards', () => {
  it('shows the wearables, emotes and experiences cards with the scene docs open by default', () => {
    renderSection()
    expect([...new Set(cards().map(card => card.getAttribute('data-card')))]).toEqual([
      'wearables',
      'emotes',
      'experiences'
    ])
    expect(activeCard()).toHaveAttribute('data-card', 'wearables')
    expect(linkNamed('About the Scene Editor')).toHaveAttribute(
      'href',
      'https://docs.decentraland.org/creator/scene-editor/get-started/about-editor'
    )
  })

  it('gives every rendered copy of a card its own panel ids, so each tab controls exactly one panel', () => {
    renderSection()
    const panelIds = screen.getAllByTestId('overview-create-tab-panel').map(panel => panel.id)
    expect(new Set(panelIds).size).toBe(panelIds.length)
    for (const tab of screen.getAllByTestId('overview-create-tab')) {
      const controlled = tab.getAttribute('aria-controls')
      const panel = tab.closest('[data-testid="overview-create-card"]')!.querySelector(`[id="${controlled}"]`)
      if (tab.getAttribute('aria-selected') === 'true') expect(panel).not.toBeNull()
    }
  })

  it('switches tabs and tracks the switch with the card and tab', () => {
    renderSection()
    const wearables = activeCard()
    fireEvent.click(within(wearables).getByRole('tab', { name: 'Smart Wearables' }))
    expect(within(wearables).getByRole('link', { name: 'Smart Wearables Docs' })).toBeInTheDocument()
    expect(within(wearables).queryByRole('link', { name: 'Creating Wearables' })).toBeNull()
    expect(track).toHaveBeenCalledWith('Click', {
      place: 'Creators Create',
      card: 'design-unique-wearables',
      tab: 'Smart Wearables'
    })
  })

  it('tracks a resource link with its card, tab and label', () => {
    renderSection()
    fireEvent.click(screen.getByRole('link', { name: 'Creating Wearables' }))
    expect(track).toHaveBeenCalledWith('Click', {
      place: 'Creators Create',
      card: 'design-unique-wearables',
      tab: 'Regular',
      title: 'Creating Wearables'
    })
  })

  it('tracks the English card, tab and link names for a Spanish-speaking visitor', () => {
    useLocale.setState({ locale: 'es' })
    renderSection()
    const link = within(activeCard()).getAllByTestId('overview-create-link')[0]
    expect(link).not.toHaveTextContent('Creating Wearables')
    fireEvent.click(link)
    expect(track).toHaveBeenCalledWith('Click', {
      place: 'Creators Create',
      card: 'design-unique-wearables',
      tab: 'Regular',
      title: 'Creating Wearables'
    })
  })

  it('offers the Creator Hub download on desktop but hides it on phones', () => {
    renderSection()
    expect(downloadLinks().length).toBeGreaterThan(0)
  })

  it('hides the desktop-only download links on phones, keeping the rest', () => {
    viewport.mobile = true
    renderSection()
    expect(downloadLinks()).toHaveLength(0)
    expect(linkNamed('About the Scene Editor')).toBeInTheDocument()
  })
})
