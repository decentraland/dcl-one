import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { FeatureFlag, getIsFeatureEnabled, resetFeatureFlagsCache } from './featureFlags'

const fetchMock = vi.fn()
vi.stubGlobal('fetch', fetchMock)

beforeEach(() => resetFeatureFlagsCache())
afterEach(() => fetchMock.mockReset())

const flags = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status })

describe('getIsFeatureEnabled', () => {
  it('reads the dapps flag file of the environment and prefixes the flag name', async () => {
    fetchMock.mockResolvedValue(flags({ flags: { 'dapps-unity-wearable-preview': true } }))
    await expect(getIsFeatureEnabled(FeatureFlag.UNITY_WEARABLE_PREVIEW)).resolves.toBe(true)
    expect(fetchMock.mock.calls[0][0]).toBe('https://feature-flags.decentraland.zone/dapps.json')
  })

  it('is off when the flag is absent, the body is malformed or the service fails', async () => {
    fetchMock.mockResolvedValueOnce(flags({ flags: {} }))
    await expect(getIsFeatureEnabled(FeatureFlag.UNITY_WEARABLE_PREVIEW)).resolves.toBe(false)
    resetFeatureFlagsCache()
    fetchMock.mockResolvedValueOnce(flags('nope', 500))
    await expect(getIsFeatureEnabled(FeatureFlag.UNITY_WEARABLE_PREVIEW)).resolves.toBe(false)
    resetFeatureFlagsCache()
    fetchMock.mockRejectedValueOnce(new Error('offline'))
    await expect(getIsFeatureEnabled(FeatureFlag.UNITY_WEARABLE_PREVIEW)).resolves.toBe(false)
  })

  it('reads each flag from the file of the application that owns it', async () => {
    fetchMock.mockImplementation((url: string) =>
      Promise.resolve(
        url.endsWith('/builder.json')
          ? flags({ flags: { 'builder-campaign': true } })
          : flags({ flags: { 'dapps-unity-wearable-preview': true } })
      )
    )
    await expect(getIsFeatureEnabled(FeatureFlag.CAMPAIGN)).resolves.toBe(true)
    await expect(getIsFeatureEnabled(FeatureFlag.UNITY_WEARABLE_PREVIEW)).resolves.toBe(true)
    expect(fetchMock.mock.calls.map(call => call[0])).toEqual([
      'https://feature-flags.decentraland.zone/builder.json',
      'https://feature-flags.decentraland.zone/dapps.json'
    ])
  })

  it('serves repeated reads from one fetch', async () => {
    fetchMock.mockResolvedValue(flags({ flags: { 'dapps-unity-wearable-preview': true } }))
    await Promise.all([
      getIsFeatureEnabled(FeatureFlag.UNITY_WEARABLE_PREVIEW),
      getIsFeatureEnabled(FeatureFlag.UNITY_WEARABLE_PREVIEW)
    ])
    await getIsFeatureEnabled(FeatureFlag.UNITY_WEARABLE_PREVIEW)
    expect(fetchMock).toHaveBeenCalledTimes(1)
  })
})
