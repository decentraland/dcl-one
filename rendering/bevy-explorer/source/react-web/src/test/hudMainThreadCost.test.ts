import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  discardHudMainThreadCostWindow,
  installHudMainThreadCostMeter,
  peekHudMainThreadCostWindow,
  recordHudReactCommit,
  takeHudMainThreadCostWindow,
  type LongAnimationFrameEntryLike
} from '../lib/hudMainThreadCost'

let nativeHud = false
vi.mock('../lib/bootMode', () => ({ isNativeHud: () => nativeHud }))

const HUD_PREFIX = 'https://cdn.example.com/bevy-web/1.0.0/assets/'
const HUD_PREFIXES = [HUD_PREFIX]
const commit = (actualDuration: number) => recordHudReactCommit('hud', 'update', actualDuration, actualDuration, 0, 0)
const frame = (startTime: number, duration: number, hudScriptMs: number): LongAnimationFrameEntryLike => ({
  startTime, duration, styleAndLayoutStart: 0,
  scripts: [{ sourceURL: HUD_PREFIX + 'app-abc123.js', duration: hudScriptMs }]
})

/** A browser that reports long animation frames: `deliver` hands entries to the observer's
 *  callback, `queue` holds them for `takeRecords`. */
class FakePerformanceObserver {
  static supportedEntryTypes = ['long-animation-frame']
  static deliver: ((entries: LongAnimationFrameEntryLike[]) => void) | null = null
  static queued: LongAnimationFrameEntryLike[] = []
  constructor(callback: (list: { getEntries: () => LongAnimationFrameEntryLike[] }) => void) {
    FakePerformanceObserver.deliver = (entries) => callback({ getEntries: () => entries })
    FakePerformanceObserver.queued = []
  }
  observe(): void {}
  takeRecords(): LongAnimationFrameEntryLike[] {
    const taken = FakePerformanceObserver.queued
    FakePerformanceObserver.queued = []
    return taken
  }
}

describe('the HUD main-thread cost meter (lib/hudMainThreadCost.ts)', () => {
  beforeEach(() => {
    nativeHud = false
    delete window.__bevyPeekHudMainThreadCost
    delete window.__bevyTakeHudMainThreadCost
    delete window.__bevyDiscardHudMainThreadCost
    discardHudMainThreadCostWindow(1000)
  })
  afterEach(() => vi.unstubAllGlobals())

  it('sums the React commits and reports the span since the last take', () => {
    installHudMainThreadCostMeter(HUD_PREFIXES, true)
    discardHudMainThreadCostWindow(1000)
    commit(1.5)
    commit(2)
    expect(takeHudMainThreadCostWindow(2000)).toMatchObject({ coveredSeconds: 1, reactCommits: { commitCount: 2, commitTimeSumMs: 3.5 } })
    expect(takeHudMainThreadCostWindow(2500)).toMatchObject({ coveredSeconds: 0.5, reactCommits: { commitCount: 0, commitTimeSumMs: 0 } })
  })

  it('reports no React commits unless the build is the profiling build', () => {
    installHudMainThreadCostMeter(HUD_PREFIXES, false)
    discardHudMainThreadCostWindow(1000)
    commit(1.5)
    expect(takeHudMainThreadCostWindow(2000)).toMatchObject({ coveredSeconds: 1, reactCommits: null })
  })

  it('leaves the window accumulating on a peek', () => {
    installHudMainThreadCostMeter(HUD_PREFIXES, true)
    discardHudMainThreadCostWindow(1000)
    commit(2)
    expect(peekHudMainThreadCostWindow(2000)).toMatchObject({ coveredSeconds: 1, reactCommits: { commitCount: 1, commitTimeSumMs: 2 } })
    commit(3)
    expect(takeHudMainThreadCostWindow(3000)).toMatchObject({ coveredSeconds: 2, reactCommits: { commitCount: 2, commitTimeSumMs: 5 } })
  })

  it('splits the observed long animation frames into the HUD script time and the style and layout time', () => {
    vi.stubGlobal('PerformanceObserver', FakePerformanceObserver)
    installHudMainThreadCostMeter(HUD_PREFIXES)
    discardHudMainThreadCostWindow(50)
    FakePerformanceObserver.deliver!([
      // the boot frame, under way when the meter was installed
      { startTime: 20, duration: 90, styleAndLayoutStart: 0, scripts: [{ sourceURL: HUD_PREFIX + 'app-abc123.js', duration: 70 }] },
      {
        startTime: 100, duration: 80, styleAndLayoutStart: 170,
        scripts: [{ sourceURL: HUD_PREFIX + 'app-abc123.js', duration: 40 }, { sourceURL: '', duration: 15 }]
      },
      // a frame that rendered nothing: no style and layout time
      { startTime: 300, duration: 60, styleAndLayoutStart: 0, scripts: [] }
    ])
    expect(takeHudMainThreadCostWindow(400).longAnimationFrames).toEqual({ frameCount: 2, hudScriptTimeSumMs: 40, styleAndLayoutTimeSumMs: 10 })
    expect(takeHudMainThreadCostWindow(500).longAnimationFrames).toEqual({ frameCount: 0, hudScriptTimeSumMs: 0, styleAndLayoutTimeSumMs: 0 })
  })

  it('counts the frames the observer still holds at a take, and a late frame in the next window', () => {
    vi.stubGlobal('PerformanceObserver', FakePerformanceObserver)
    installHudMainThreadCostMeter(HUD_PREFIXES)
    discardHudMainThreadCostWindow(0)
    // ended before the take, its entry not yet delivered
    FakePerformanceObserver.queued = [frame(100, 80, 40)]
    expect(takeHudMainThreadCostWindow(1000).longAnimationFrames).toEqual({ frameCount: 1, hudScriptTimeSumMs: 40, styleAndLayoutTimeSumMs: 0 })
    // the frame the take ran in: began before the new window, delivered after
    FakePerformanceObserver.deliver!([frame(990, 120, 30)])
    expect(takeHudMainThreadCostWindow(2000).longAnimationFrames).toEqual({ frameCount: 1, hudScriptTimeSumMs: 30, styleAndLayoutTimeSumMs: 0 })
  })

  it('drops the frames that began before a discard, delivered or not', () => {
    vi.stubGlobal('PerformanceObserver', FakePerformanceObserver)
    installHudMainThreadCostMeter(HUD_PREFIXES)
    FakePerformanceObserver.queued = [frame(100, 80, 40)]
    discardHudMainThreadCostWindow(1000)
    FakePerformanceObserver.deliver!([frame(900, 150, 30), frame(1100, 60, 10)])
    expect(takeHudMainThreadCostWindow(2000).longAnimationFrames).toEqual({ frameCount: 1, hudScriptTimeSumMs: 10, styleAndLayoutTimeSumMs: 0 })
  })

  it('counts a script under any of the HUD prefixes', () => {
    vi.stubGlobal('PerformanceObserver', FakePerformanceObserver)
    installHudMainThreadCostMeter(['http://localhost:5173/src/', 'http://localhost:5173/@fs/'])
    discardHudMainThreadCostWindow(0)
    FakePerformanceObserver.deliver!([
      {
        startTime: 100, duration: 80, styleAndLayoutStart: 0,
        scripts: [
          { sourceURL: 'http://localhost:5173/src/main.tsx', duration: 20 },
          { sourceURL: 'http://localhost:5173/@fs/library/ui/index.ts', duration: 5 },
          { sourceURL: 'http://localhost:5173/engine/engine.js', duration: 30 }
        ]
      }
    ])
    expect(takeHudMainThreadCostWindow(400).longAnimationFrames).toEqual({ frameCount: 1, hudScriptTimeSumMs: 25, styleAndLayoutTimeSumMs: 0 })
  })

  it('discards what accumulated and restarts the span', () => {
    installHudMainThreadCostMeter(HUD_PREFIXES, true)
    commit(5)
    discardHudMainThreadCostWindow(4000)
    expect(takeHudMainThreadCostWindow(4250)).toMatchObject({ coveredSeconds: 0.25, reactCommits: { commitCount: 0, commitTimeSumMs: 0 } })
  })

  it('installs the page hooks without the observer where the browser has no long animation frames', () => {
    installHudMainThreadCostMeter(HUD_PREFIXES)
    expect(typeof window.__bevyPeekHudMainThreadCost).toBe('function')
    expect(typeof window.__bevyTakeHudMainThreadCost).toBe('function')
    expect(typeof window.__bevyDiscardHudMainThreadCost).toBe('function')
    expect(window.__bevyTakeHudMainThreadCost!().longAnimationFrames).toBeNull()
  })

  it('does nothing on the native HUD', () => {
    nativeHud = true
    installHudMainThreadCostMeter(HUD_PREFIXES)
    expect(window.__bevyTakeHudMainThreadCost).toBeUndefined()
  })
})
