// The React HUD's page main-thread cost, accumulated into one window: the HUD root's React
// commits, where the HUD is React's profiling build, and, where the browser reports them, the
// page's long animation frames (over 50 ms) whose entries arrived inside the window, with the
// HUD bundle's script time in them and their style and layout time. A long frame's entry
// arrives after the frame ends, so a window is read with the observer's pending entries pulled
// in first, and an entry that arrives later lands in the next window. Frames that began before
// the install, or before the last discard (the tab shown again), are not counted.
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

/** The part of PerformanceObserver the meter uses. */
interface LongAnimationFrameObserverLike {
  takeRecords(): ArrayLike<unknown>
}

declare global {
  interface Window {
    /** The current window, left accumulating. */
    __bevyPeekHudMainThreadCost?: () => HudMainThreadCostWindow
    /** The current window; the next one starts. */
    __bevyTakeHudMainThreadCost?: () => HudMainThreadCostWindow
    /** Ends the current window and ignores the frames that began before now. */
    __bevyDiscardHudMainThreadCost?: () => void
  }
}

const LONG_ANIMATION_FRAME_ENTRY = 'long-animation-frame'

let windowStartedAt = 0
/** A long frame that began before this (ms) is not counted: the install, or the last discard. */
let ignoreFramesBefore = 0
let reactCommitsReported = false
let reactCommitSums: ReactCommitSums = { commitCount: 0, commitTimeSumMs: 0 }
let longAnimationFrameObserver: LongAnimationFrameObserverLike | null = null
let longAnimationFrameSums: LongAnimationFrameSums = { frameCount: 0, hudScriptTimeSumMs: 0, styleAndLayoutTimeSumMs: 0 }
let hudScriptPrefixes: readonly string[] = []

export const recordHudReactCommit: ProfilerOnRenderCallback = (_id, _phase, actualDuration) => {
  reactCommitSums.commitCount += 1
  reactCommitSums.commitTimeSumMs += actualDuration
}

/** Adds the frames that began at or after `ignoreFramesBefore`; a frame's scripts under one of
 *  `hudScriptPrefixes` count as HUD script time. */
function recordLongAnimationFrames(entries: ArrayLike<unknown>): void {
  for (const entry of Array.from(entries) as LongAnimationFrameEntryLike[]) {
    if (entry.startTime < ignoreFramesBefore) continue
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
}

/** The window since the last take or discard, up to `now` (ms), with the observer's pending
 *  entries pulled in; the window keeps accumulating. */
export function peekHudMainThreadCostWindow(now: number): HudMainThreadCostWindow {
  if (longAnimationFrameObserver) recordLongAnimationFrames(longAnimationFrameObserver.takeRecords())
  return {
    coveredSeconds: (now - windowStartedAt) / 1000,
    reactCommits: reactCommitsReported ? { ...reactCommitSums } : null,
    longAnimationFrames: longAnimationFrameObserver ? { ...longAnimationFrameSums } : null
  }
}

/** The window as `peekHudMainThreadCostWindow` reads it, then starts the next one at `now`
 *  (ms); an entry arriving later for a frame that began inside this window counts in the next. */
export function takeHudMainThreadCostWindow(now: number): HudMainThreadCostWindow {
  const taken = peekHudMainThreadCostWindow(now)
  resetHudMainThreadCostWindow(now)
  return taken
}

/** Drops what accumulated and the observer's pending entries, starts the next window at `now`
 *  (ms) and ignores the frames that began before it. */
export function discardHudMainThreadCostWindow(now: number): void {
  longAnimationFrameObserver?.takeRecords()
  ignoreFramesBefore = now
  resetHudMainThreadCostWindow(now)
}

function resetHudMainThreadCostWindow(now: number): void {
  windowStartedAt = now
  reactCommitSums = { commitCount: 0, commitTimeSumMs: 0 }
  longAnimationFrameSums = { frameCount: 0, hudScriptTimeSumMs: 0, styleAndLayoutTimeSumMs: 0 }
}

/** No-op on the native HUD. Reports React commits only when `reactCommitsReportedByBuild`
 *  (the profiling build). Observes the page's long animation frames where the browser reports
 *  them, those begun from the install on, and exposes the window to the page as
 *  `window.__bevyPeekHudMainThreadCost`, `window.__bevyTakeHudMainThreadCost` and
 *  `window.__bevyDiscardHudMainThreadCost`. */
export function installHudMainThreadCostMeter(
  prefixes: readonly string[],
  reactCommitsReportedByBuild: boolean = HUD_REACT_COMMITS_REPORTED
): void {
  if (isNativeHud()) return
  hudScriptPrefixes = prefixes
  reactCommitsReported = reactCommitsReportedByBuild
  longAnimationFrameObserver = null
  if (typeof PerformanceObserver !== 'undefined' && PerformanceObserver.supportedEntryTypes.includes(LONG_ANIMATION_FRAME_ENTRY)) {
    const observer = new PerformanceObserver((entries) => recordLongAnimationFrames(entries.getEntries()))
    observer.observe({ type: LONG_ANIMATION_FRAME_ENTRY })
    longAnimationFrameObserver = observer
  }
  discardHudMainThreadCostWindow(performance.now())
  window.__bevyPeekHudMainThreadCost = () => peekHudMainThreadCostWindow(performance.now())
  window.__bevyTakeHudMainThreadCost = () => takeHudMainThreadCostWindow(performance.now())
  window.__bevyDiscardHudMainThreadCost = () => discardHudMainThreadCostWindow(performance.now())
}
