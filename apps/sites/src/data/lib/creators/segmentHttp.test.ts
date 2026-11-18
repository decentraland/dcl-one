import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { postToSegment, type SegmentHttpInput } from './segmentHttp'

const input = (overrides: Partial<SegmentHttpInput> = {}): SegmentHttpInput => ({
  writeKey: 'write-key-example',
  call: { type: 'track', event: 'Click' },
  properties: { place: 'Creators Hero' },
  anonymousId: 'anon-1',
  apiHost: 'api.example.com/v1',
  app: { name: 'creators-website', version: '1.2.3' },
  search: '?utm_source=x',
  ...overrides
})

async function sentBody(beacon: ReturnType<typeof vi.fn>): Promise<Record<string, unknown>> {
  const blob = beacon.mock.calls[0][1] as Blob
  return JSON.parse(await blob.text()) as Record<string, unknown>
}

let beacon: ReturnType<typeof vi.fn>

beforeEach(() => {
  beacon = vi.fn(() => true)
  Object.defineProperty(navigator, 'sendBeacon', { value: beacon, configurable: true })
})

afterEach(() => {
  vi.unstubAllGlobals()
  Object.defineProperty(navigator, 'sendBeacon', { value: undefined, configurable: true })
})

describe('postToSegment', () => {
  it('beacons a track call in the Tracking API shape, keyed in the body', async () => {
    postToSegment(input({ userId: '0xabc' }))

    expect(beacon).toHaveBeenCalledWith('https://api.example.com/v1/track', expect.any(Blob))
    expect((beacon.mock.calls[0][1] as Blob).type).toBe('text/plain')
    const body = await sentBody(beacon)
    expect(body).toMatchObject({
      writeKey: 'write-key-example',
      event: 'Click',
      userId: '0xabc',
      anonymousId: 'anon-1',
      integrations: {},
      properties: { place: 'Creators Hero' },
      context: {
        direct: true,
        page: { path: '/', search: '?utm_source=x', url: window.location.href },
        userAgent: navigator.userAgent,
        library: { name: 'creators-website-beacon' },
        app: { name: 'creators-website', version: '1.2.3' }
      }
    })
    expect(body.messageId).toEqual(expect.stringMatching(/^creators-website-beacon-/))
    expect(Number.isNaN(Date.parse(body.timestamp as string))).toBe(false)
  })

  it('carries the campaign context analytics.js derives from the utm params', async () => {
    postToSegment(input({ search: '?utm_source=partner&utm_campaign=Spring%20Drop&utm_org=dcl&ref=x' }))
    expect((await sentBody(beacon)).context).toMatchObject({
      campaign: { source: 'partner', name: 'Spring Drop', org: 'dcl' }
    })
    beacon.mockClear()
    postToSegment(input({ search: '' }))
    expect((await sentBody(beacon)).context).not.toHaveProperty('campaign')
  })

  it('sends an anonymous visitor without a userId', async () => {
    postToSegment(input())
    expect(await sentBody(beacon)).not.toHaveProperty('userId')
  })

  it('posts a page view to the page endpoint with the page fields as properties', async () => {
    postToSegment(input({ call: { type: 'page', name: '/create' }, properties: { source: 'creators-website' } }))

    expect(beacon).toHaveBeenCalledWith('https://api.example.com/v1/page', expect.any(Blob))
    const body = await sentBody(beacon)
    expect(body).toMatchObject({
      name: '/create',
      properties: { path: '/', search: '?utm_source=x', source: 'creators-website' }
    })
    expect(body).not.toHaveProperty('event')
  })

  it('falls back to a keepalive fetch when the beacon is refused or unavailable', () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(null))
    vi.stubGlobal('fetch', fetchMock)
    beacon.mockReturnValue(false)

    postToSegment(input())

    expect(fetchMock).toHaveBeenCalledWith(
      'https://api.example.com/v1/track',
      expect.objectContaining({ method: 'POST', keepalive: true, credentials: 'omit' })
    )
  })

  it('never throws, even when every transport fails', () => {
    beacon.mockImplementation(() => {
      throw new Error('beacon blocked')
    })
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('Failed to fetch')))
    expect(() => postToSegment(input())).not.toThrow()
  })
})
