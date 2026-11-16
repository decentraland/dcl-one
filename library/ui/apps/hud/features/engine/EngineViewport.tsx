// A transparent placeholder that carves out a screen region for the engine to render
// into (the rich map, or the avatar preview). It measures its own on-screen rect and
// reports it to the scene over the bridge; the scene draws the engine view there, behind
// the React DOM, showing through this transparent hole. Clears the rect on unmount.

import { useLayoutEffect, useRef } from 'react'
import { subscribeHudScale } from '@explorer-core/lib/hudScale'
import styles from './EngineViewport.module.css'
import type { EngineViewportRegion } from '@explorer-core/engine/protocol'

type Rect = { x: number; y: number; width: number; height: number }

export function EngineViewport({
  region,
  report
}: {
  region: EngineViewportRegion
  /** Stable callback (useCallback) — reports the rect (in CSS pixels) and the current
   *  `devicePixelRatio`, or null to clear. */
  report: (region: EngineViewportRegion, rect: Rect | null, dpr?: number) => void
}): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null)

  useLayoutEffect(() => {
    const el = ref.current
    if (!el) return
    const push = (): void => {
      const r = el.getBoundingClientRect()
      // Re-read the ratio on every push rather than once: dragging the window to a display with
      // a different density changes it.
      report(region, { x: r.left, y: r.top, width: r.width, height: r.height }, window.devicePixelRatio)
    }
    push()
    // The rect moves for two independent reasons, and each needs its own signal:
    //  • layout — a panel resizing around this element, or the page scrolling;
    //  • the HUD's `--ui-scale`, which is a CSS TRANSFORM. A transform changes no border box,
    //    so ResizeObserver never fires for it. Subscribing to the scale (rather than to `resize`,
    //    which fires before the scale is applied) is what keeps the reported rect from going
    //    stale, which used to leave the engine drawing the minimap and the avatar preview at
    //    their pre-resize size and offset.
    const ro = new ResizeObserver(push)
    ro.observe(el)
    const unsubscribe = subscribeHudScale(push)
    // Only a scroll of something holding this element moves it: a list scrolling beside it does not.
    const onScroll = (e: Event): void => {
      if (e.target instanceof Node && !e.target.contains(el)) return
      push()
    }
    window.addEventListener('scroll', onScroll, true)
    return () => {
      ro.disconnect()
      unsubscribe()
      window.removeEventListener('scroll', onScroll, true)
      report(region, null)
    }
  }, [region, report])

  return <div ref={ref} className={styles.viewport} aria-hidden="true" />
}
