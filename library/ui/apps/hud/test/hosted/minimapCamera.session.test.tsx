// The minimap's Camera style is drawn by the engine behind the page. The session says when that view
// is up: the map's hole stays shut until then, since it would show the player's own view.
import { act } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { enterAsGuest, renderSession } from '../harness'

const RECT = { x: 12, y: 60, width: 220, height: 220 }
const config = (style: 'satellite' | 'imposters') => ({ style, rotation: 'north' as const, visibleMeters: 256 })

afterEach(() => {
  vi.useRealTimers()
})

describe('minimap camera in the session', () => {
  it('is ready once the scene says its camera is drawing, for the Camera style with a place to draw', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    expect(h.session().minimap.cameraReady).toBe(false)

    act(() => h.session().setEngineViewport('map', RECT))
    act(() => h.session().minimap.setConfig(config('imposters')))
    expect(h.driver.last('minimapConfig')).toEqual({ kind: 'minimapConfig', ...config('imposters') })
    expect(h.session().minimap.cameraReady).toBe(false)

    act(() => h.driver.emit({ kind: 'minimapReady' }))
    expect(h.session().minimap.cameraReady).toBe(true)
    // a zoom step or a resize keeps the same camera
    act(() => h.session().minimap.setConfig({ ...config('imposters'), visibleMeters: 128 }))
    act(() => h.session().setEngineViewport('map', { ...RECT, width: 240 }))
    expect(h.session().minimap.cameraReady).toBe(true)
  })

  it('waits again for the next camera: after another style, and after the map was collapsed', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().setEngineViewport('map', RECT))
    act(() => h.session().minimap.setConfig(config('imposters')))
    act(() => h.driver.emit({ kind: 'minimapReady' }))

    act(() => h.session().minimap.setConfig(config('satellite')))
    expect(h.session().minimap.cameraReady).toBe(false)
    act(() => h.session().minimap.setConfig(config('imposters')))
    expect(h.session().minimap.cameraReady).toBe(false)
    act(() => h.driver.emit({ kind: 'minimapReady' }))
    expect(h.session().minimap.cameraReady).toBe(true)

    act(() => h.session().setEngineViewport('map', null))
    expect(h.session().minimap.cameraReady).toBe(false)
    act(() => h.session().setEngineViewport('map', RECT))
    expect(h.session().minimap.cameraReady).toBe(false)
  })

  it('ignores a camera announced for a style the page no longer shows', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().setEngineViewport('map', RECT))
    act(() => h.session().minimap.setConfig(config('satellite')))
    act(() => h.driver.emit({ kind: 'minimapReady' }))
    expect(h.session().minimap.cameraReady).toBe(false)
  })

  it('opens anyway when the scene never answers, and not for a camera no longer wanted', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    vi.useFakeTimers()
    act(() => h.session().setEngineViewport('map', RECT))
    act(() => h.session().minimap.setConfig(config('imposters')))
    act(() => vi.advanceTimersByTime(2999))
    expect(h.session().minimap.cameraReady).toBe(false)
    act(() => vi.advanceTimersByTime(1))
    expect(h.session().minimap.cameraReady).toBe(true)

    act(() => h.session().minimap.setConfig(config('satellite')))
    act(() => h.session().minimap.setConfig(config('imposters')))
    act(() => h.session().minimap.setConfig(config('satellite')))
    act(() => vi.advanceTimersByTime(5000))
    expect(h.session().minimap.cameraReady).toBe(false)
  })
})
