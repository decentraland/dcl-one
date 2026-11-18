import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { BodyPartCategory, BodyShape, WearableCategory } from '@dcl/schemas'
import {
  LivePreviewError,
  buildDefinition,
  buildStateUrl,
  fetchBridgeState,
  isSameModelMetadata,
  queryLocalNetworkPermission,
  resolveBridgeUrl,
  type BridgeState
} from './livePreview'

describe('resolveBridgeUrl', () => {
  it('trusts only local origins from the query param', () => {
    expect(resolveBridgeUrl(null)).toBe('http://localhost:8080')
    expect(resolveBridgeUrl('8081')).toBe('http://localhost:8081')
    expect(resolveBridgeUrl('http://127.0.0.1:9000/')).toBe('http://127.0.0.1:9000')
    expect(resolveBridgeUrl('https://evil.example.com')).toBe('http://localhost:8080')
    expect(resolveBridgeUrl('not a url')).toBe('http://localhost:8080')
    expect(resolveBridgeUrl('99999')).toBe('http://localhost:8080')
    expect(resolveBridgeUrl('0')).toBe('http://localhost:8080')
  })
})

describe('buildStateUrl', () => {
  it('long-polls with the last seen version once there is one', () => {
    expect(buildStateUrl('http://127.0.0.1:8080/', null)).toBe('http://127.0.0.1:8080/state')
    expect(buildStateUrl('http://127.0.0.1:8080', 7)).toBe('http://127.0.0.1:8080/state?since=7')
  })
})

describe('fetchBridgeState', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('rejects a payload without a version as invalid', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(JSON.stringify({ name: 'x' }))))
    await expect(fetchBridgeState('http://localhost:8080')).rejects.toMatchObject({ code: 'invalid_state' })
  })

  it('reports the bridge status on a failed response', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(null, { status: 503 })))
    const error = await fetchBridgeState('http://localhost:8080').catch((e: unknown) => e)
    expect(error).toBeInstanceOf(LivePreviewError)
    expect(error).toMatchObject({ code: 'bridge_response', status: 503 })
  })
})

describe('isSameModelMetadata', () => {
  it('ignores the version counter but not the category', () => {
    expect(
      isSameModelMetadata({ version: 1, type: 'emote', name: 'Wave' }, { version: 2, type: 'emote', name: 'Wave' })
    ).toBe(true)
    expect(
      isSameModelMetadata(
        { version: 1, type: 'wearable', category: 'hat' },
        { version: 2, type: 'wearable', category: 'mask' }
      )
    ).toBe(false)
    expect(isSameModelMetadata(null, { version: 1 })).toBe(false)
    expect(
      isSameModelMetadata(
        { version: 1, bodyShapes: [BodyShape.MALE, BodyShape.FEMALE] },
        { version: 2, bodyShapes: [BodyShape.FEMALE, BodyShape.MALE] }
      )
    ).toBe(true)
  })
})

describe('buildDefinition', () => {
  const glb = new Blob([new Uint8Array([1, 2, 3])])
  const wearable = (category: string, version = 1): BridgeState => ({ version, type: 'wearable', name: 'x', category })

  it('changes the id with every push so the renderers reload the model', () => {
    expect(buildDefinition(wearable('hat', 1), glb).id).not.toBe(buildDefinition(wearable('hat', 2), glb).id)
  })

  it('removes the default hand hiding for upper bodies and when hiding the upper body', () => {
    const upper = buildDefinition(wearable(WearableCategory.UPPER_BODY), glb)
    const hat = buildDefinition(wearable(WearableCategory.HAT), glb, { hides: [WearableCategory.UPPER_BODY] })
    const plain = buildDefinition(wearable(WearableCategory.HAT), glb)
    expect('data' in upper && upper.data.removesDefaultHiding).toEqual([BodyPartCategory.HANDS])
    expect('data' in hat && hat.data.removesDefaultHiding).toEqual([BodyPartCategory.HANDS])
    expect('data' in plain && plain.data.removesDefaultHiding).toEqual([])
  })

  it('lets the panel override the bridge category and mirrors hides into the representation', () => {
    const definition = buildDefinition(wearable('hat'), glb, { category: 'mask', hides: ['hair'] })
    expect('data' in definition && definition.data.category).toBe('mask')
    expect('data' in definition && definition.data.representations[0].overrideHides).toEqual(['hair'])
  })

  it('builds emotes from the bridge type or an emote category, with loop in the id', () => {
    const byType = buildDefinition({ version: 1, type: 'emote', name: 'Wave' }, glb, { loop: false })
    const byCategory = buildDefinition({ version: 1, category: 'dance' }, glb)
    expect('emoteDataADR74' in byType && byType.emoteDataADR74.loop).toBe(false)
    expect(byType.id).toMatch(/-once$/)
    expect('emoteDataADR74' in byCategory && byCategory.emoteDataADR74.loop).toBe(true)
  })
})

describe('queryLocalNetworkPermission', () => {
  const query = vi.fn()
  let permissions: PropertyDescriptor | undefined

  beforeEach(() => {
    permissions = Object.getOwnPropertyDescriptor(navigator, 'permissions')
    Object.defineProperty(navigator, 'permissions', { value: { query }, configurable: true })
  })
  afterEach(() => {
    query.mockReset()
    if (permissions) Object.defineProperty(navigator, 'permissions', permissions)
    else delete (navigator as { permissions?: unknown }).permissions
  })

  it('does not ask when the page itself is served from localhost', async () => {
    await expect(queryLocalNetworkPermission('localhost')).resolves.toBeNull()
    expect(query).not.toHaveBeenCalled()
  })

  it('falls back to the loopback permission name when the legacy one is unknown', async () => {
    const status = { state: 'denied' }
    query.mockRejectedValueOnce(new TypeError('unknown name')).mockResolvedValueOnce(status)
    await expect(queryLocalNetworkPermission('builder.example.com')).resolves.toBe(status)
    expect(query).toHaveBeenLastCalledWith({ name: 'loopback-network' })
  })

  it('resolves to null when the browser has no such permission', async () => {
    query.mockRejectedValue(new TypeError('unknown name'))
    await expect(queryLocalNetworkPermission('builder.example.com')).resolves.toBeNull()
  })
})
