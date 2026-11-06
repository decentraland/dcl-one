import { describe, it, expect } from 'vitest'
import { act } from '@testing-library/react'
import type { PhotoModeSnapshot } from '../engine/protocol'
import { renderSession, enterAsGuest } from './harness'
import { normalizedFrameSize, FRAME_SCALE, PHOTO_ASPECT } from '../features/photoMode/frame'

// DOMAIN: photo mode (camera reel capture) — the engine owns the camera; the session relays its
// state and sends the buttons.
describe('photo mode domain', () => {
  const snapshot = (over: Partial<PhotoModeSnapshot> = {}): PhotoModeSnapshot => ({
    active: true,
    hudHidden: false,
    cursorLocked: true,
    phase: 'idle',
    storageCurrent: 1,
    storageMax: 500,
    storageLoaded: true,
    event: 'none',
    eventSeq: 0,
    previewJpegBase64: '',
    message: '',
    ...over
  })

  it('toggle asks the engine to open, then to close, photo mode', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    expect(h.session().photoMode.active).toBe(false)
    act(() => h.session().photoMode.toggle())
    expect(h.driver.last('setPhotoMode')).toEqual({ kind: 'setPhotoMode', enable: true })
    h.driver.emit({ kind: 'photoMode', state: snapshot() })
    expect(h.session().photoMode.active).toBe(true)
    act(() => h.session().photoMode.toggle())
    expect(h.driver.last('setPhotoMode')).toEqual({ kind: 'setPhotoMode', enable: false })
  })

  it('opening photo mode closes the open page', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().gallery.toggle())
    expect(h.session().gallery.open).toBe(true)
    h.driver.emit({ kind: 'photoMode', state: snapshot() })
    expect(h.session().gallery.open).toBe(false)
  })

  it('a capture is kept for the overlay and dropped when photo mode closes', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit({ kind: 'photoMode', state: snapshot({ event: 'captured', eventSeq: 1, previewJpegBase64: 'AAAA' }) })
    expect(h.session().photoMode.capture).toEqual({ seq: 1, preview: 'AAAA' })
    h.driver.emit({ kind: 'photoMode', state: snapshot({ event: 'none', eventSeq: 1 }) })
    expect(h.session().photoMode.capture).toEqual({ seq: 1, preview: 'AAAA' })
    h.driver.emit({ kind: 'photoMode', state: snapshot({ active: false, eventSeq: 1 }) })
    expect(h.session().photoMode.capture).toBeNull()
  })

  it('take sends the photo request', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit({ kind: 'photoMode', state: snapshot() })
    act(() => h.session().photoMode.take())
    expect(h.driver.last('takePhoto')).toEqual({ kind: 'takePhoto' })
  })

  it('the frame keeps 16:9 at 0.87 of the tighter screen dimension', () => {
    expect(normalizedFrameSize(PHOTO_ASPECT)).toEqual({ x: FRAME_SCALE, y: FRAME_SCALE })
    const wide = normalizedFrameSize(21 / 9)
    expect(wide.y).toBe(FRAME_SCALE)
    expect(wide.x).toBeCloseTo((FRAME_SCALE * PHOTO_ASPECT) / (21 / 9))
    const tall = normalizedFrameSize(1)
    expect(tall.x).toBe(FRAME_SCALE)
    expect(tall.y).toBeCloseTo(FRAME_SCALE / PHOTO_ASPECT)
  })
})
