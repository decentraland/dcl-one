import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  discardHudMainThreadCostWindow,
  installHudMainThreadCostMeter,
  recordHudReactCommit,
  takeHudMainThreadCostWindow,
  type LongAnimationFrameEntryLike
} from '../lib/hudMainThreadCost'

let nativeHud = false
vi.mock('../lib/bootMode', () => ({ isNativeHud: () => nativeHud }))

const HUD_PREFIX = 'https://cdn.example.com/bevy-web/1.0.0/assets/'
const commit = (actualDuration: number) => recordHudReactCommit('hud', 'update', actualDuration, actualDuration, 0, 0)

/** A browser that reports long animation frames: the observer gets the entries handed to `report`. */
class FakePerformanceObserver {
  static supportedEntryTypes = ['long-animation-frame']
  static report: ((entries: LongAnimationFrameEntryLike[]) => void) | null = null
  constructor(callback: (list: { getEntries: () => LongAnimationFrameEntryLike[] }) => void) {
    FakePerformanceObserver.report = (entries) => callback({ getEntries: () => entries })
  }
  observe(): void {}
}

describe('the HUD main-thread cost meter (lib/hudMainThreadCost.ts)', () => {
  beforeEach(() => {
    nativeHud = false
    delete window.__bevyTakeHudMainThreadCost
    delete window.__bevyDiscardHudMainThreadCost
    discardHudMainThreadCostWindow(1000)
  })
  afterEach(() => vi.unstubAllGlobals())

  it('sums the React commits and reports the span since the last take', () => {
    commit(1.5)
    commit(2)
    expect(takeHudMainThreadCostWindow(2000)).toMatchObject({ coveredSeconds: 1, reactCommitCount: 2, reactCommitTimeSumMs: 3.5 })
    expect(takeHudMainThreadCostWindow(2500)).toMatchObject({ coveredSeconds: 0.5, reactCommitCount: 0, reactCommitTimeSumMs: 0 })
  })

  it('splits the observed long animation frames into the HUD script time and the style and layout time', () => {
    vi.stubGlobal('PerformanceObserver', FakePerformanceObserver)
    installHudMainThreadCostMeter(HUD_PREFIX)
    FakePerformanceObserver.report!([
      {
        startTime: 100, duration: 80, styleAndLayoutStart: 170,
        scripts: [{ sourceURL: HUD_PREFIX + 'app-abc123.js', duration: 40 }, { sourceURL: '', duration: 15 }]
      },
      // a frame that rendered nothing: no style and layout time
      { startTime: 300, duration: 60, styleAndLayoutStart: 0, scripts: [] }
    ])
    expect(window.__bevyTakeHudMainThreadCost!().longAnimationFrames).toEqual({ frameCount: 2, hudScriptTimeSumMs: 40, styleAndLayoutTimeSumMs: 10 })
    expect(window.__bevyTakeHudMainThreadCost!().longAnimationFrames).toEqual({ frameCount: 0, hudScriptTimeSumMs: 0, styleAndLayoutTimeSumMs: 0 })
  })

  it('discards what accumulated and restarts the span', () => {
    commit(5)
    discardHudMainThreadCostWindow(4000)
    expect(takeHudMainThreadCostWindow(4250)).toMatchObject({ coveredSeconds: 0.25, reactCommitCount: 0, reactCommitTimeSumMs: 0 })
  })

  it('installs the page hooks without the observer where the browser has no long animation frames', () => {
    installHudMainThreadCostMeter(HUD_PREFIX)
    expect(typeof window.__bevyTakeHudMainThreadCost).toBe('function')
    expect(typeof window.__bevyDiscardHudMainThreadCost).toBe('function')
    expect(window.__bevyTakeHudMainThreadCost!().longAnimationFrames).toBeNull()
  })

  it('does nothing on the native HUD', () => {
    nativeHud = true
    installHudMainThreadCostMeter(HUD_PREFIX)
    expect(window.__bevyTakeHudMainThreadCost).toBeUndefined()
  })
})
