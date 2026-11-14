import { afterEach, describe, expect, it, vi } from 'vitest'
import { featureFlagsUrl, featureFlagsUserId, fetchFeatureFlags, parseFeatureFlags } from './featureFlags'
import { getPref, PREF } from './prefs'

afterEach(() => vi.unstubAllGlobals())

describe('parseFeatureFlags', () => {
  it('keeps only explorer-bevy- flags and variants, the prefix dropped', () => {
    const parsed = parseFeatureFlags({
      flags: { 'explorer-bevy-photo-mode': true, 'Explorer-Bevy-voice': true, 'explorer-bevy-off': false, 'explorer-bevy-': true },
      variants: { 'explorer-bevy-onboarding': { name: 'greetings', enabled: true, payload: { type: 'string', value: 'hi' } } }
    })
    expect(parsed.flags).toEqual({ 'photo-mode': true, voice: true, off: false })
    expect(parsed.variants).toEqual({ onboarding: { name: 'greetings', enabled: true, payload: { type: 'string', value: 'hi' } } })
  })

  it('leaves out the other explorer flags in the file', () => {
    const parsed = parseFeatureFlags({
      flags: { 'explorer-alfa-camera-reel': true, 'explorer-bevy-a': true, 'dapps-x': true, bevy: true },
      variants: { 'explorer-onboarding': { name: 'greetings', enabled: true, payload: null } }
    })
    expect(parsed).toEqual({ flags: { a: true }, variants: {} })
  })

  it('reads a flag on only where the file says true, and survives a malformed file', () => {
    expect(parseFeatureFlags({ flags: { 'explorer-bevy-a': 'true', 'explorer-bevy-b': 1 } }).flags).toEqual({ a: false, b: false })
    expect(parseFeatureFlags(null)).toEqual({ flags: {}, variants: {} })
    expect(parseFeatureFlags({ flags: null, variants: { 'explorer-bevy-x': null } })).toEqual({ flags: {}, variants: {} })
  })

  it('keeps a variant without a usable payload, payload null', () => {
    const parsed = parseFeatureFlags({ variants: { 'explorer-bevy-x': { name: 'disabled', enabled: false } } })
    expect(parsed.variants.x).toEqual({ name: 'disabled', enabled: false, payload: null })
  })
})

describe('fetchFeatureFlags', () => {
  it('asks the base domain service for explorer.json with the debug and rollout-id headers', async () => {
    const fetchMock = vi.fn(async () => Response.json({ flags: { 'explorer-bevy-a': true }, variants: {} }))
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
