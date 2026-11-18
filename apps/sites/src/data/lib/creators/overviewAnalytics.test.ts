import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { sendOverviewPage, sendOverviewTrack } from '~/lib/overviewSegment'
import { OverviewSection, clickPayload, trackClick, trackOverviewPage, trackSectionViewed } from './overviewAnalytics'

vi.mock('~/lib/overviewSegment', () => ({ sendOverviewTrack: vi.fn(), sendOverviewPage: vi.fn() }))
const mac = vi.hoisted(() => ({ arch: null as 'intel' | 'apple_silicon' | 'unknown' | null }))
vi.mock('~/lib/creatorHubDownload', () => ({ macArchHint: () => mac.arch }))

beforeEach(() => {
  vi.clearAllMocks()
  mac.arch = null
})
afterEach(() => window.history.replaceState({}, '', '/'))

describe('trackSectionViewed', () => {
  it('sends the section under the name the sites landing used', () => {
    trackSectionViewed(OverviewSection.WHY, true)
    expect(sendOverviewTrack).toHaveBeenCalledWith('Section Viewed', { section_viewed: 'Creators Why', mobile: true })
  })
})

describe('trackOverviewPage', () => {
  it('names the view after the page’s path on the site', () => {
    trackOverviewPage()
    expect(sendOverviewPage).toHaveBeenCalledWith('/create')
  })
})

describe('clickPayload', () => {
  it('reads the place, title, card and tab dimensions off the element', () => {
    const element = document.createElement('a')
    element.setAttribute('data-place', 'Creators Create')
    element.setAttribute('data-title', 'Creating Wearables')
    element.setAttribute('data-card', 'design-unique-wearables')
    element.setAttribute('data-tab', 'Regular')
    element.setAttribute('href', 'https://example.com')
    expect(clickPayload(element)).toEqual({
      place: 'Creators Create',
      title: 'Creating Wearables',
      card: 'design-unique-wearables',
      tab: 'Regular'
    })
  })

  it('names a download click the way sites did', () => {
    const element = document.createElement('a')
    element.setAttribute('data-place', 'Creators Hero')
    element.setAttribute('data-event', 'Download')
    element.setAttribute('data-download-target', 'creator_hub')
    expect(clickPayload(element)).toEqual({ place: 'Creators Hero', event: 'Download', download_target: 'creator_hub' })
  })

  it("adds the Mac's chip to a download click, and only to one", () => {
    mac.arch = 'intel'
    const download = document.createElement('a')
    download.setAttribute('data-download-target', 'creator_hub')
    expect(clickPayload(download)).toMatchObject({ mac_arch: 'intel' })
    const other = document.createElement('a')
    other.setAttribute('data-place', 'Creators Why')
    expect(clickPayload(other)).not.toHaveProperty('mac_arch')
  })

  it('skips empty values, a redundant Click subtype and the data attributes styled primitives stamp', () => {
    const element = document.createElement('button')
    element.setAttribute('data-place', 'Creators Faqs')
    element.setAttribute('data-title', '')
    element.setAttribute('data-event', 'Click')
    element.setAttribute('data-testid', 'faq-row')
    element.setAttribute('data-variant', 'primary')
    expect(clickPayload(element)).toEqual({ place: 'Creators Faqs' })
  })

  it('attributes the click to the campaign of the link the visitor came in on', () => {
    window.history.replaceState({}, '', '/?utm_source=SheFi&utm_campaign=Creator%20Week')
    const element = document.createElement('a')
    element.setAttribute('data-place', 'Creators Why')
    element.setAttribute('data-title', 'Join a Community of Creators')
    expect(clickPayload(element)).toEqual({
      utm_source: 'shefi',
      utm_campaign: 'creator_week',
      place: 'Creators Why',
      title: 'Join a Community of Creators'
    })
  })
})

describe('trackClick', () => {
  it('reports a Click with the clicked element as the payload source', () => {
    const element = document.createElement('a')
    element.setAttribute('data-place', 'Creators Connect')
    element.setAttribute('data-title', 'join-discord')
    trackClick({ currentTarget: element } as unknown as React.SyntheticEvent<Element>)
    expect(sendOverviewTrack).toHaveBeenCalledWith('Click', { place: 'Creators Connect', title: 'join-discord' })
  })
})
