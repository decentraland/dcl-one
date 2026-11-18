import { afterEach, describe, expect, it, vi } from 'vitest'
import { PreviewRenderer } from '@dcl/schemas'
import { pickRenderer } from './pickRenderer'

type Nav = {
  connection?: { downlink?: number; saveData?: boolean }
  deviceMemory?: number
  hardwareConcurrency?: number
}

function stubNavigator(nav: Nav) {
  for (const [key, value] of Object.entries(nav)) {
    Object.defineProperty(navigator, key, { value, configurable: true })
  }
}

afterEach(() => {
  for (const key of ['connection', 'deviceMemory', 'hardwareConcurrency']) {
    Object.defineProperty(navigator, key, { value: undefined, configurable: true })
  }
  Object.defineProperty(window, 'devicePixelRatio', { value: 1, configurable: true })
  vi.restoreAllMocks()
})

const capable = { override: null, unityEnabled: true, measuredMbps: null } as const

describe('pickRenderer', () => {
  it('forces Babylon from the URL override and when the flag is off', () => {
    expect(pickRenderer({ override: 'babylon', unityEnabled: true, measuredMbps: null })).toEqual({
      renderer: PreviewRenderer.BABYLON,
      reason: 'url-override'
    })
    expect(pickRenderer({ override: null, unityEnabled: false, measuredMbps: null }).reason).toBe('flag-off')
  })

  it('stays optimistic with Unity when nothing about the device or the link is known', () => {
    expect(pickRenderer(capable)).toEqual({ renderer: PreviewRenderer.UNITY, reason: 'optimistic-default' })
    // The browser's own downlink guess is not a reading: it misfires on warm caches.
    stubNavigator({ connection: { downlink: 2 } })
    expect(pickRenderer(capable).reason).toBe('optimistic-default')
  })

  it('does not downgrade on a phone-sized viewport', () => {
    Object.defineProperty(window, 'matchMedia', { value: () => ({ matches: true }), configurable: true })
    expect(pickRenderer(capable).renderer).toBe(PreviewRenderer.UNITY)
  })

  it('degrades to Babylon on data saver, a slow link, low memory or a hi-DPI screen with few cores', () => {
    stubNavigator({ connection: { saveData: true } })
    expect(pickRenderer(capable).reason).toBe('save-data')
    stubNavigator({ connection: {} })
    expect(pickRenderer({ ...capable, measuredMbps: 2 }).reason).toBe('slow-connection')
    expect(pickRenderer({ ...capable, measuredMbps: 0 }).reason).toBe('slow-connection')
    stubNavigator({ deviceMemory: 2 })
    expect(pickRenderer({ ...capable, measuredMbps: 50 }).reason).toBe('low-device-memory')
    stubNavigator({ deviceMemory: 8, hardwareConcurrency: 4 })
    Object.defineProperty(window, 'devicePixelRatio', { value: 2, configurable: true })
    expect(pickRenderer({ ...capable, measuredMbps: 50 }).reason).toBe('gpu-capability')
    stubNavigator({ hardwareConcurrency: 12 })
    expect(pickRenderer({ ...capable, measuredMbps: 50 })).toEqual({
      renderer: PreviewRenderer.UNITY,
      reason: 'connection-ok'
    })
  })
})
