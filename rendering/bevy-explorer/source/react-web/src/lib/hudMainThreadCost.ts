// The React HUD's page main-thread cost, accumulated into one window: the HUD root's React
// commits, where the HUD is React's profiling build, and, where the browser reports them, the
// page's long animation frames (over 50 ms) that began inside the window, with the HUD bundle's
// script time in them and their style and layout time.
import type { ProfilerOnRenderCallback } from 'react'
import { isNativeHud } from './bootMode'

/** The HUD is React's profiling build (`HUD_REACT_PROFILING=1` at build time), the production
 *  build in which <Profiler> reports commits. Off, React's production build reports none. */
export const HUD_REACT_COMMITS_REPORTED: boolean =
  typeof __HUD_REACT_PROFILING__ !== 'undefined' && __HUD_REACT_PROFILING__

export interface ReactCommitSums {
  commitCount: number
  commitTimeSumMs: number
}

export interface LongAnimationFrameSums {
  frameCount: number
  hudScriptTimeSumMs: number
  styleAndLayoutTimeSumMs: number
}

export interface HudMainThreadCostWindow {
  coveredSeconds: number
  /** null unless the meter was installed reporting React commits. */
  reactCommits: ReactCommitSums | null
  /** null when the browser has no Long Animation Frame timing. */
  longAnimationFrames: LongAnimationFrameSums | null
}

/** The part of PerformanceLongAnimationFrameTiming the meter reads. */
export interface LongAnimationFrameEntryLike {
  startTime: number
  duration: number
  styleAndLayoutStart: number
  scripts: ReadonlyArray<{ sourceURL: string; duration: number }>
}

declare global {
  interface Window {
    __bevyTakeHudMainThreadCost?: () => HudMainThreadCostWindow
    __bevyDiscardHudMainThreadCost?: () => void
  }
}

const LONG_ANIMATION_FRAME_ENTRY = 'long-animation-frame'

let windowStartedAt = 0
let reactCommitsReported = false
let reactCommitSums: ReactCommitSums = { commitCount: 0, commitTimeSumMs: 0 }
let longAnimationFramesSupported = false
let longAnimationFrameSums: LongAnimationFrameSums = { frameCount: 0, hudScriptTimeSumMs: 0, styleAndLayoutTimeSumMs: 0 }

export const recordHudReactCommit: ProfilerOnRenderCallback = (_id, _phase, actualDuration) => {
  reactCommitSums.commitCount += 1
  reactCommitSums.commitTimeSumMs += actualDuration
}

/** Adds the frame; its scripts under one of `hudScriptPrefixes` count as HUD script time. */
function recordLongAnimationFrame(entry: LongAnimationFrameEntryLike, hudScriptPrefixes: readonly string[]): void {
  longAnimationFrameSums.frameCount += 1
  for (const script of entry.scripts) {
    if (hudScriptPrefixes.some((prefix) => script.sourceURL.startsWith(prefix))) {
      longAnimationFrameSums.hudScriptTimeSumMs += script.duration
    }
  }
  if (entry.styleAndLayoutStart > 0) {
    longAnimationFrameSums.styleAndLayoutTimeSumMs += entry.startTime + entry.duration - entry.styleAndLayoutStart
  }
}

/** The window since the last take or discard, then starts the next one at `now` (ms). */
export function takeHudMainThreadCostWindow(now: number): HudMainThreadCostWindow {
  const taken: HudMainThreadCostWindow = {
    coveredSeconds: (now - windowStartedAt) / 1000,
    reactCommits: reactCommitsReported ? { ...reactCommitSums } : null,
    longAnimationFrames: longAnimationFramesSupported ? { ...longAnimationFrameSums } : null
  }
  discardHudMainThreadCostWindow(now)
  return taken
}

/** Drops what accumulated and starts the next window at `now` (ms). */
export function discardHudMainThreadCostWindow(now: number): void {
  windowStartedAt = now
  reactCommitSums = { commitCount: 0, commitTimeSumMs: 0 }
  longAnimationFrameSums = { frameCount: 0, hudScriptTimeSumMs: 0, styleAndLayoutTimeSumMs: 0 }
}

/** No-op on the native HUD. Reports React commits only when `reactCommitsReportedByBuild`
 *  (the profiling build). Observes the page's long animation frames where the browser reports
 *  them, from the install on, and exposes the window to the page as
 *  `window.__bevyTakeHudMainThreadCost` and `window.__bevyDiscardHudMainThreadCost`. */
export function installHudMainThreadCostMeter(
  hudScriptPrefixes: readonly string[],
  reactCommitsReportedByBuild: boolean = HUD_REACT_COMMITS_REPORTED
): void {
  if (isNativeHud()) return
  reactCommitsReported = reactCommitsReportedByBuild
  discardHudMainThreadCostWindow(performance.now())
  longAnimationFramesSupported =
    typeof PerformanceObserver !== 'undefined' && PerformanceObserver.supportedEntryTypes.includes(LONG_ANIMATION_FRAME_ENTRY)
  if (longAnimationFramesSupported) {
    new PerformanceObserver((entries) => {
      for (const entry of entries.getEntries() as unknown as LongAnimationFrameEntryLike[]) {
        // a frame that began before the window is not counted
        if (entry.startTime < windowStartedAt) continue
        recordLongAnimationFrame(entry, hudScriptPrefixes)
      }
    }).observe({ type: LONG_ANIMATION_FRAME_ENTRY })
  }
  window.__bevyTakeHudMainThreadCost = () => takeHudMainThreadCostWindow(performance.now())
  window.__bevyDiscardHudMainThreadCost = () => discardHudMainThreadCostWindow(performance.now())
}
