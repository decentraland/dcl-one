import { afterEach, describe, it, expect } from 'vitest'
import { act, render } from '@testing-library/react'
import type { PhotoModeSnapshot } from '@hud/engine/protocol'
import { renderSession, enterAsGuest, type Harness } from './harness'
import { normalizedFrameSize, FRAME_SCALE, PHOTO_ASPECT } from '@hud/features/photoMode/frame'
import { openPopup, resetPopups, PopupHost } from '@hud/design'

afterEach(() => {
  resetPopups()
  document.body.innerHTML = ''
})

const action = (name: string): { kind: 'systemAction'; action: string; pressed: boolean } => ({
  kind: 'systemAction',
  action: name,
  pressed: true
})

// DOMAIN: photo mode (camera reel capture) — the engine owns the camera; the session opens and
// closes it (the key, the rail, the gallery key), declares it as focus and sends the buttons.
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

  const open = async (h: Harness): Promise<void> => {
    h.driver.emit({ kind: 'photoMode', state: snapshot() })
    expect(h.session().photoMode.active).toBe(true)
  }

  it('set asks the engine to open, then to close, photo mode, naming what asked', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    expect(h.session().photoMode.active).toBe(false)
    act(() => h.session().photoMode.set(true, 'button'))
    expect(h.driver.last('setPhotoMode')).toEqual({ kind: 'setPhotoMode', enable: true, source: 'button' })
    await open(h)
    act(() => h.session().photoMode.set(false, 'button'))
    expect(h.driver.last('setPhotoMode')).toEqual({ kind: 'setPhotoMode', enable: false, source: 'button' })
  })

  it('the photo mode key opens it as a shortcut, and closes it again', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit(action('PhotoMode'))
    expect(h.driver.last('setPhotoMode')).toEqual({ kind: 'setPhotoMode', enable: true, source: 'shortcut' })
    await open(h)
    h.driver.emit(action('PhotoMode'))
    expect(h.driver.last('setPhotoMode')).toEqual({ kind: 'setPhotoMode', enable: false, source: 'shortcut' })
  })

  it('the key is ignored with a popup open, like every hotkey', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    render(<PopupHost />)
    act(() => {
      openPopup(() => <div>a popup</div>)
    })
    h.driver.emit(action('PhotoMode'))
    expect(h.driver.last('setPhotoMode')).toBeUndefined()
  })

  it('photo mode declares focus, and the other hotkeys stand down while it is on', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    const before = h.driver.sentOf('uiFocus').length
    await open(h)
    const focus = h.driver.sentOf('uiFocus')
    expect(focus.length).toBeGreaterThan(before)
    expect(focus[focus.length - 1].ui).toBe(true)
    h.driver.emit(action('Settings'))
    expect(h.session().settings.open).toBe(false)
    h.driver.emit({ kind: 'photoMode', state: snapshot({ active: false }) })
    const after = h.driver.sentOf('uiFocus')
    expect(after[after.length - 1].ui).toBe(false)
  })

  it('the gallery key closes photo mode on its way to the gallery', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    await open(h)
    h.driver.emit(action('Gallery'))
    expect(h.driver.last('setPhotoMode')).toEqual({ kind: 'setPhotoMode', enable: false, source: 'shortcut' })
    expect(h.session().gallery.open).toBe(true)
  })

  it('a popup opening closes photo mode', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    await open(h)
    render(<PopupHost />)
    act(() => {
      openPopup(() => <div>a popup</div>)
    })
    expect(h.driver.last('setPhotoMode')).toEqual({ kind: 'setPhotoMode', enable: false, source: 'button' })
  })


  it('opening photo mode closes the open page', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().gallery.toggle())
    expect(h.session().gallery.open).toBe(true)
    await open(h)
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
