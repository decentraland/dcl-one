import { describe, it, expect, vi } from 'vitest'
import { act } from '@testing-library/react'
import { renderSession, enterAsGuest } from '../harness'

// DOMAIN: avatarPreview — the engine renders the player's avatar (a TextureCamera)
// into a screen rect the React Backpack carves out. The page's only API is reporting
// that rect via engineViewport(region:'avatarPreview', rect|null).
describe('avatarPreview domain', () => {
  it('reports the avatar-preview rect to the scene', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    const rect = { x: 100, y: 50, width: 300, height: 500 }
    act(() => h.session().setEngineViewport('avatarPreview', rect))
    expect(h.driver.last('engineViewport')).toEqual({
      kind: 'engineViewport',
      region: 'avatarPreview',
      rect
    })
  })

  it('reports the part of the rect the avatar is framed in, when the page gives one', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    const rect = { x: 0, y: 0, width: 1184, height: 1184 }
    const focus = { x: 304, y: 160, width: 576, height: 886 }
    act(() => h.session().setEngineViewport('avatarPreview', rect, 2, focus))
    expect(h.driver.last('engineViewport')).toEqual({ kind: 'engineViewport', region: 'avatarPreview', rect, dpr: 2, focus })
  })

  it('reports the box the page shows the preview in, when the page gives one', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    const rect = { x: 68, y: -35, width: 957, height: 957 }
    const clip = { x: 313, y: 32, width: 1293, height: 808 }
    act(() => h.session().setEngineViewport('avatarPreview', rect, 1, undefined, clip))
    expect(h.driver.last('engineViewport')).toEqual({ kind: 'engineViewport', region: 'avatarPreview', rect, dpr: 1, clip })
  })

  it('clears the rect (null) when the preview closes', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().setEngineViewport('avatarPreview', null))
    expect(h.driver.last('engineViewport')).toEqual({
      kind: 'engineViewport',
      region: 'avatarPreview',
      rect: null
    })
  })

  it('is ready once the scene has drawn the avatar, and not again until the next opening says so', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    const rect = { x: 100, y: 50, width: 300, height: 500 }
    expect(h.session().avatarPreviewReady).toBe(false)
    act(() => h.session().setEngineViewport('avatarPreview', rect))
    expect(h.session().avatarPreviewReady).toBe(false)
    act(() => h.driver.emit({ kind: 'avatarPreviewReady' }))
    expect(h.session().avatarPreviewReady).toBe(true)
    // a re-report of the open preview (a resize) keeps it shown
    act(() => h.session().setEngineViewport('avatarPreview', { ...rect, width: 320 }))
    expect(h.session().avatarPreviewReady).toBe(true)
    act(() => h.session().setEngineViewport('avatarPreview', null))
    expect(h.session().avatarPreviewReady).toBe(false)
    act(() => h.session().setEngineViewport('avatarPreview', rect))
    expect(h.session().avatarPreviewReady).toBe(false)
  })

  it('ignores a ready the scene sends while no preview is open', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.driver.emit({ kind: 'avatarPreviewReady' }))
    expect(h.session().avatarPreviewReady).toBe(false)
  })

  it('shows the preview anyway when the scene never says it is ready', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    vi.useFakeTimers()
    try {
      act(() => h.session().setEngineViewport('avatarPreview', { x: 100, y: 50, width: 300, height: 500 }))
      act(() => vi.advanceTimersByTime(2999))
      expect(h.session().avatarPreviewReady).toBe(false)
      act(() => vi.advanceTimersByTime(1))
      expect(h.session().avatarPreviewReady).toBe(true)
      // closed before the wait ran out: nothing turns it on later
      act(() => h.session().setEngineViewport('avatarPreview', null))
      act(() => h.session().setEngineViewport('avatarPreview', { x: 100, y: 50, width: 300, height: 500 }))
      act(() => h.session().setEngineViewport('avatarPreview', null))
      act(() => vi.advanceTimersByTime(5000))
      expect(h.session().avatarPreviewReady).toBe(false)
    } finally {
      vi.useRealTimers()
    }
  })

  it('turns the preview avatar and zooms its camera as the page asks', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().backpack.rotate(-20))
    expect(h.driver.last('previewRotate')).toEqual({ kind: 'previewRotate', degrees: -20 })
    act(() => h.session().backpack.zoom(1.5, 0.25))
    expect(h.driver.last('previewZoom')).toEqual({ kind: 'previewZoom', zoom: 1.5, anchor: 0.25 })
    act(() => h.session().turnLobbyAvatar(12))
    expect(h.driver.last('previewRotate')).toEqual({ kind: 'previewRotate', degrees: 12, stage: 'lobby' })
  })
})
