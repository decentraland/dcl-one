import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest'
import { getHudScale, installHudScale, subscribeHudScale } from '../lib/hudScale'

const cssScale = (): string => document.documentElement.style.getPropertyValue('--ui-scale')

// the frames the page asked for, run by hand
const frames: FrameRequestCallback[] = []
function runFrame(): void {
  for (const frame of frames.splice(0)) frame(0)
}
function setViewport(height: number, width = height * 3): void {
  Object.defineProperty(window, 'innerWidth', { value: width, configurable: true })
  Object.defineProperty(window, 'innerHeight', { value: height, configurable: true })
  window.dispatchEvent(new Event('resize'))
}
function resizeTo(height: number, width?: number): void {
  setViewport(height, width)
  runFrame()
}

beforeAll(() => {
  vi.stubGlobal('requestAnimationFrame', (frame: FrameRequestCallback) => frames.push(frame))
  installHudScale()
})
afterAll(() => vi.unstubAllGlobals())

// DOMAIN: hudScale — `--ui-scale` follows the viewport against a 1920x1080 reference (smaller ratio wins) with no clamp
// over the sizes a window takes (0.25 and 4 only keep the value finite), and the
// engine cutouts (minimap, avatar preview) re-measure off it. The ordering below is the whole
// point: the HUD is scaled with a CSS transform, so a subscriber that measures the DOM has to
// run after the property is written or it reports a rect from the pre-resize layout.
describe('hud scale', () => {
  it('follows the viewport height at every window size', () => {
    resizeTo(1080)
    expect(cssScale()).toBe('1.0000')
    resizeTo(810)
    expect(cssScale()).toBe('0.7500')
    resizeTo(480) // a 480-high window: no floor above it
    expect(cssScale()).toBe('0.4444')
    resizeTo(1440)
    expect(cssScale()).toBe('1.3333')
    resizeTo(2160) // 4K: no ceiling below it
    expect(cssScale()).toBe('2.0000')
    expect(getHudScale()).toBe(2)
  })

  it('stays finite in a collapsed or enormous frame', () => {
    resizeTo(0)
    expect(cssScale()).toBe('0.2500')
    resizeTo(8640)
    expect(cssScale()).toBe('4.0000')
  })

  it('uses the smaller of the width and height ratios, so the canvas expands along the longer axis', () => {
    resizeTo(960, 1536) // 16:10: width-bound (height alone would give 0.889)
    expect(cssScale()).toBe('0.8000')
    resizeTo(1200, 1920) // 1920x1200: one canvas px per CSS px, extra height
    expect(cssScale()).toBe('1.0000')
    resizeTo(1080, 1280) // narrow window
    expect(cssScale()).toBe('0.6667')
    resizeTo(1080, 900) // narrower than the old 0.6 floor
    expect(cssScale()).toBe('0.4688')
  })

  it('writes --ui-scale before waking subscribers, in the same frame', () => {
    resizeTo(1080)
    const seen: string[] = []
    const off = subscribeHudScale(() => seen.push(cssScale()))
    resizeTo(648)
    off()
    expect(seen).toEqual(['0.6000'])
  })

  it('does not wake subscribers when the scale is unchanged', () => {
    resizeTo(1080)
    let woken = 0
    const off = subscribeHudScale(() => woken++)
    resizeTo(1080) // same height: no change, no wake
    resizeTo(3000) // 1 -> 2.7778: one wake
    off()
    expect(woken).toBe(1)
  })

  it('applies once per frame however many resize events arrive before it', () => {
    resizeTo(1080)
    const seen: string[] = []
    const off = subscribeHudScale(() => seen.push(cssScale()))
    for (const height of [1000, 900, 800, 700, 648]) setViewport(height)
    // nothing is written or announced until the frame
    expect(frames).toHaveLength(1)
    expect(cssScale()).toBe('1.0000')
    expect(seen).toEqual([])
    runFrame()
    expect(seen).toEqual(['0.6000'])
    // the next resize asks for a frame of its own
    setViewport(1080)
    expect(frames).toHaveLength(1)
    runFrame()
    off()
    expect(seen).toEqual(['0.6000', '1.0000'])
  })
})
