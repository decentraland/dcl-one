import { afterEach, describe, expect, it, vi } from 'vitest'
import { pending } from './analytics'
import {
  featureFlag,
  featureFlagOverride,
  featureFlagOverridesString,
  featureFlagsSettled,
  featureFlagsString,
  featureFlagsUrl,
  featureFlagsUserId,
  featureFlagVariant,
  fetchFeatureFlags,
  loadFeatureFlags,
  parseFeatureFlags,
  parseFlagOverrides,
  primeFeatureFlags,
  setFlagOverrides,
  shared,
  type Flag
} from './featureFlags'
import { getPref, PREF } from './prefs'

const NO_OVERRIDES = parseFlagOverrides('')
// These tests read made-up names, not FLAG's.
const flag = (name: string) => name as Flag

afterEach(() => {
  vi.unstubAllGlobals()
  setFlagOverrides(NO_OVERRIDES)
  primeFeatureFlags(null)
})

describe('parseFlagOverrides', () => {
  it('reads names forced on, -names forced off and -* to ignore the service', () => {
    const o = parseFlagOverrides('?ff=photo-mode,-voice&ff=-*')
    expect([...o.on]).toEqual([['photo-mode', null]])
    expect([...o.off]).toEqual(['voice'])
    expect(o.ignoreAll).toBe(true)
  })

  it('reads name:variant as forced on with that variant, the variant as written', () => {
    const o = parseFlagOverrides('?ff=Onboarding:Greetings,plain:')
    expect([...o.on]).toEqual([
      ['onboarding', 'Greetings'],
      ['plain', null]
    ])
  })

  it('lets the last mention of a name win, and skips empty items', () => {
    const o = parseFlagOverrides('?ff=a,-a,,-,b&ff=-b,b')
    expect([...o.on]).toEqual([['b', null]])
    expect([...o.off]).toEqual(['a'])
    expect(o.ignoreAll).toBe(false)
  })
})

describe('overrides', () => {
  it('force a flag over the answer, either way, and drop a forced-off flag’s variant', () => {
    primeFeatureFlags({ flags: { a: false, b: true }, variants: { b: { name: 'v', enabled: true, payload: null } } })
    setFlagOverrides(parseFlagOverrides('?ff=a,-b'))
    expect(featureFlag(flag('a'))).toBe(true)
    expect(featureFlag(flag('b'))).toBe(false)
    expect(featureFlagVariant(flag('b'))).toBeNull()
  })

  it('with -* leave every flag off but the forced ones, and ask the service nothing', async () => {
    const fetchMock = vi.fn()
    vi.stubGlobal('fetch', fetchMock)
    setFlagOverrides(parseFlagOverrides('?ff=-*,a'))
    await loadFeatureFlags()
    expect(fetchMock).not.toHaveBeenCalled()
    expect(featureFlagsSettled()).toBe(true)
    expect(featureFlag(flag('a'))).toBe(true)
    expect(featureFlag(flag('b'))).toBe(false)
    expect(featureFlagOverride(flag('b'))).toBe(false)
  })

  it('force a variant: with the answer\u2019s payload when it is that variant, without one otherwise', () => {
    const payload = { type: 'string', value: 'hi' }
    primeFeatureFlags({ flags: { a: true, b: true }, variants: { a: { name: 'x', enabled: true, payload }, b: { name: 'x', enabled: true, payload } } })
    setFlagOverrides(parseFlagOverrides('?ff=a:x,b:y,c:z'))
    expect(featureFlagVariant(flag('a'))).toEqual({ name: 'x', enabled: true, payload })
    expect(featureFlagVariant(flag('b'))).toEqual({ name: 'y', enabled: true, payload: null })
    expect(featureFlag(flag('c'))).toBe(true)
    expect(featureFlagVariant(flag('c'))).toEqual({ name: 'z', enabled: true, payload: null })
  })

  it('leave an unnamed flag to the service', () => {
    setFlagOverrides(parseFlagOverrides('?ff=a'))
    expect(featureFlagOverride(flag('b'))).toBeUndefined()
  })
})

describe('the flags for analytics', () => {
  it('list the flags that are on, sorted, each with its enabled variant, as a ?ff= value', () => {
    primeFeatureFlags({
      flags: { voice: true, off: false, 'photo-mode': true, ab: true },
      variants: { ab: { name: 'b', enabled: true, payload: null }, voice: { name: 'disabled', enabled: false, payload: null } }
    })
    expect(featureFlagsString()).toBe('-*,ab:b,photo-mode,voice')
    expect(featureFlagOverridesString()).toBe('')
  })

  it('apply the overrides, and give them on their own', () => {
    primeFeatureFlags({ flags: { voice: true, 'photo-mode': true }, variants: {} })
    setFlagOverrides(parseFlagOverrides('?ff=-voice,extra,ab:b'))
    expect(featureFlagsString()).toBe('-*,ab:b,extra,photo-mode')
    expect(featureFlagOverridesString()).toBe('extra,ab:b,-voice')
  })

  it('reproduce the session when pasted back as ?ff=', () => {
    primeFeatureFlags({ flags: { voice: true, ab: true }, variants: { ab: { name: 'b', enabled: true, payload: null } } })
    const recorded = featureFlagsString()
    primeFeatureFlags({ flags: { other: true }, variants: {} })
    setFlagOverrides(parseFlagOverrides(`?ff=${recorded}`))
    expect(featureFlag(flag('voice'))).toBe(true)
    expect(featureFlagVariant(flag('ab'))?.name).toBe('b')
    expect(featureFlag(flag('other'))).toBe(false)
    expect(featureFlagsString()).toBe(recorded)
  })

  it('are tracked once the read settles, with where they came from', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => Response.json({ flags: { 'explorer-bevy-a': true }, variants: {} })))
    await loadFeatureFlags()
    const resolved = pending().filter((e) => e.event === 'Feature Flags Resolved').at(-1)
    expect(resolved?.properties).toMatchObject({ feature_flags: '-*,a', feature_flags_override: '', feature_flags_source: 'service' })
  })

  it('say so when the service could not be read or was ignored', async () => {
    vi.spyOn(console, 'warn').mockImplementation(() => undefined)
    vi.stubGlobal('fetch', vi.fn(async () => new Response('nope', { status: 503 })))
    await loadFeatureFlags()
    expect(pending().at(-1)?.properties).toMatchObject({ feature_flags: '-*', feature_flags_source: 'unavailable' })
    primeFeatureFlags(null)
    setFlagOverrides(parseFlagOverrides('?ff=-*,a'))
    await loadFeatureFlags()
    expect(pending().at(-1)?.properties).toMatchObject({ feature_flags: '-*,a', feature_flags_override: '-*,a', feature_flags_source: 'ignored' })
    vi.restoreAllMocks()
  })
})

describe('parseFeatureFlags', () => {
  it('keeps a flag shared with the other client under its own name, only once FLAG lists it', () => {
    const file = { flags: { 'explorer-shared-thing': true }, variants: { 'explorer-shared-thing': { name: 'v', enabled: true } } }
    expect(parseFeatureFlags(file)).toEqual({ flags: {}, variants: {} })
    shared('shared-thing')
    expect(parseFeatureFlags(file)).toEqual({ flags: { 'shared-thing': true }, variants: { 'shared-thing': { name: 'v', enabled: true, payload: null } } })
  })


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
