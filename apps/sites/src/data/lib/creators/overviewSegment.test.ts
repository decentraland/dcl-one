import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const settings = vi.hoisted((): { values: Record<string, string> } => ({ values: {} }))
vi.mock('~/config', () => ({ config: { get: (key: string, fallback = '') => settings.values[key] ?? fallback } }))

const analytics = vi.hoisted(() => ({ sendDirect: vi.fn() }))
vi.mock('~/lib/analytics', () => ({
  IS_BOT: false,
  sendDirect: analytics.sendDirect,
  eventProperties: (props: Record<string, unknown> = {}) => ({ source: 'creators-website', ...props }),
  resolveAnalyticsUrl: (url: string) => (url ? new URL(url) : undefined),
  stampApp: vi.fn()
}))

const idle = vi.hoisted(() => ({ run: undefined as (() => void) | undefined }))
vi.mock('~/lib/idle', () => ({
  afterLoadIdle: (callback: () => void) => {
    idle.run = callback
    return () => undefined
  }
}))

const segment = vi.hoisted(() => ({
  instance: { track: vi.fn(), page: vi.fn() },
  load: vi.fn(),
  settle: undefined as ((ok: boolean) => void) | undefined
}))
vi.mock('@segment/analytics-next', () => ({ AnalyticsBrowser: { load: segment.load } }))

const SITES_KEY = 'sites-write-key-example'
const AT = new Date('2026-09-25T12:00:00Z').getTime()

const loadSender = () => import('./overviewSegment')

// Starts sites' analytics.js and resolves (or fails) it on demand.
async function bootAnalytics(ok: boolean) {
  idle.run?.()
  await vi.waitFor(() => expect(segment.load).toHaveBeenCalled())
  segment.settle?.(ok)
  await vi.waitFor(() =>
    expect(
      analytics.sendDirect.mock.calls.length +
        segment.instance.track.mock.calls.length +
        segment.instance.page.mock.calls.length
    ).toBeGreaterThan(0)
  )
}

beforeEach(() => {
  vi.resetModules()
  settings.values = {
    SITES_SEGMENT_API_KEY: SITES_KEY,
    SEGMENT_ANALYTICS_URL: 'https://evs.example.com/x/y.min.js',
    SEGMENT_API_HOST: 'api.example.com/v1'
  }
  idle.run = undefined
  analytics.sendDirect.mockClear()
  segment.instance.track.mockClear()
  segment.instance.page.mockClear()
  segment.load.mockReset().mockImplementation(() => {
    const ready = new Promise((resolve, reject) => {
      segment.settle = ok => (ok ? resolve([segment.instance, {}]) : reject(new Error('blocked')))
    })
    return Object.assign(ready, { addSourceMiddleware: vi.fn() })
  })
  vi.useFakeTimers({ now: AT, toFake: ['Date'] })
})

afterEach(() => {
  vi.useRealTimers()
})

describe('overview sender', () => {
  it("loads sites' analytics.js with sites' key behind the first-party proxy", async () => {
    const { sendOverviewPage } = await loadSender()
    sendOverviewPage('/create')
    idle.run?.()
    await vi.waitFor(() =>
      expect(segment.load).toHaveBeenCalledWith(
        { writeKey: SITES_KEY, cdnURL: 'https://evs.example.com' },
        {
          integrations: {
            'Segment.io': { apiHost: 'api.example.com/v1', deliveryStrategy: { config: { keepalive: true } } }
          }
        }
      )
    )
  })

  it("delivers early calls through analytics.js once it is ready, so sites' browser destinations see them", async () => {
    const { sendOverviewPage, sendOverviewTrack } = await loadSender()
    sendOverviewPage('/create')
    sendOverviewTrack('Section Viewed', { section_viewed: 'Creators Why' })
    expect(analytics.sendDirect).not.toHaveBeenCalled()

    await bootAnalytics(true)
    expect(segment.instance.page).toHaveBeenCalledWith('/create', { source: 'creators-website' })
    expect(segment.instance.track).toHaveBeenCalledWith(
      'Section Viewed',
      expect.objectContaining({ section_viewed: 'Creators Why', track_called_at: AT, track_deferred: true })
    )

    sendOverviewTrack('Click', { place: 'Creators Hero' })
    expect(segment.instance.track).toHaveBeenLastCalledWith(
      'Click',
      expect.objectContaining({ place: 'Creators Hero', track_deferred: false })
    )
    expect(analytics.sendDirect).not.toHaveBeenCalled()
  })

  it('sends what is still queued over the HTTP API when the visitor leaves first', async () => {
    const { sendOverviewPage, sendOverviewTrack } = await loadSender()
    sendOverviewPage('/create')
    sendOverviewTrack('Click', { place: 'Creators Hero' })

    window.dispatchEvent(new Event('pagehide'))
    expect(analytics.sendDirect).toHaveBeenCalledWith(SITES_KEY, { type: 'page', name: '/create' })
    expect(analytics.sendDirect).toHaveBeenCalledWith(
      SITES_KEY,
      { type: 'track', event: 'Click' },
      {
        place: 'Creators Hero',
        track_called_at: AT,
        track_delivered_at: AT,
        track_deferred: true
      }
    )
  })

  it('stops waiting for analytics.js after a timeout, or when the page is hidden', async () => {
    vi.useFakeTimers({ now: AT })
    const { sendOverviewTrack } = await loadSender()
    sendOverviewTrack('Click', { place: 'Creators Hero' })

    Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'hidden' })
    document.dispatchEvent(new Event('visibilitychange'))
    Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'visible' })
    expect(analytics.sendDirect).toHaveBeenCalledTimes(1)

    idle.run?.()
    sendOverviewTrack('Section Viewed', { section_viewed: 'Creators Why' })
    expect(analytics.sendDirect).toHaveBeenCalledTimes(1)
    vi.advanceTimersByTime(10_000)
    expect(analytics.sendDirect).toHaveBeenLastCalledWith(
      SITES_KEY,
      { type: 'track', event: 'Section Viewed' },
      expect.objectContaining({ track_deferred: true })
    )
  })

  it('sends later calls through analytics.js when it loads after the timeout', async () => {
    vi.useFakeTimers({ now: AT })
    const { sendOverviewPage, sendOverviewTrack } = await loadSender()
    sendOverviewPage('/create')
    idle.run?.()
    await vi.waitFor(() => expect(segment.load).toHaveBeenCalled())
    vi.advanceTimersByTime(10_000)
    // The page view already left over the HTTP API; GA4 counts its own page view when it loads.
    expect(analytics.sendDirect).toHaveBeenCalledWith(SITES_KEY, { type: 'page', name: '/create' })

    segment.settle?.(true)
    await vi.waitFor(() => {
      sendOverviewTrack('Click', { place: 'Creators Hero' })
      expect(segment.instance.track).toHaveBeenCalled()
    })
    expect(segment.instance.page).not.toHaveBeenCalled()
  })

  it('falls back to the HTTP API for good when analytics.js cannot load', async () => {
    const { sendOverviewPage, sendOverviewTrack } = await loadSender()
    sendOverviewPage('/create')
    await bootAnalytics(false)
    expect(analytics.sendDirect).toHaveBeenCalledWith(SITES_KEY, { type: 'page', name: '/create' })

    sendOverviewTrack('Click', { place: 'Creators Hero' })
    expect(analytics.sendDirect).toHaveBeenLastCalledWith(
      SITES_KEY,
      { type: 'track', event: 'Click' },
      expect.objectContaining({ place: 'Creators Hero', track_deferred: true })
    )
    expect(segment.instance.track).not.toHaveBeenCalled()
  })
})
