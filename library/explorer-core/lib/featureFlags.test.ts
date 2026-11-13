import { afterEach, describe, expect, it, vi } from 'vitest'
import { featureFlagsUrl, featureFlagsUserId, fetchFeatureFlags, parseFeatureFlags } from './featureFlags'
import { getPref, PREF } from './prefs'

afterEach(() => vi.unstubAllGlobals())

describe('parseFeatureFlags', () => {
  it('drops the explorer- prefix from flags and variants', () => {
    const parsed = parseFeatureFlags({
      flags: { 'explorer-alfa-camera-reel': true, 'Explorer-voice-chat': true, 'explorer-off': false },
      variants: { 'explorer-onboarding': { name: 'greetings', enabled: true, payload: { type: 'string', value: 'hi' } } }
    })
    expect(parsed.flags).toEqual({ 'alfa-camera-reel': true, 'voice-chat': true, off: false })
    expect(parsed.variants).toEqual({ onboarding: { name: 'greetings', enabled: true, payload: { type: 'string', value: 'hi' } } })
  })

  it('reads a flag on only where the file says true, and survives a malformed file', () => {
    expect(parseFeatureFlags({ flags: { 'explorer-a': 'true', 'explorer-b': 1 } }).flags).toEqual({ a: false, b: false })
    expect(parseFeatureFlags(null)).toEqual({ flags: {}, variants: {} })
    expect(parseFeatureFlags({ flags: null, variants: { 'explorer-x': null } })).toEqual({ flags: {}, variants: {} })
  })

  it('keeps a variant without a usable payload, payload null', () => {
    const parsed = parseFeatureFlags({ variants: { 'explorer-x': { name: 'disabled', enabled: false } } })
    expect(parsed.variants.x).toEqual({ name: 'disabled', enabled: false, payload: null })
  })
})

describe('fetchFeatureFlags', () => {
  it('asks the base domain service for explorer.json with the debug and rollout-id headers', async () => {
    const fetchMock = vi.fn(async () => Response.json({ flags: { 'explorer-a': true }, variants: {} }))
    vi.stubGlobal('fetch', fetchMock)
    const flags = await fetchFeatureFlags(undefined, featureFlagsUrl('decentraland.zone'))
    expect(flags.flags).toEqual({ a: true })
    const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit]
    expect(url).toBe('https://feature-flags.decentraland.zone/explorer.json')
    expect(init.headers).toEqual({ 'X-Debug': 'false', 'X-Address-Hash': featureFlagsUserId() })
  })

  it('fails on an error status, so every flag reads off', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response('nope', { status: 503 })))
    await expect(fetchFeatureFlags()).rejects.toThrow('HTTP 503')
  })
})

describe('featureFlagsUserId', () => {
  it('mints one anonymous id and keeps it', () => {
    const first = featureFlagsUserId()
    expect(first).toMatch(/^[0-9a-f-]{36}$/)
    expect(featureFlagsUserId()).toBe(first)
    expect(getPref(PREF.featureFlagsUserId)).toBe(first)
  })
})
