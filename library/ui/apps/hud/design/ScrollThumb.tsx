// ScrollThumb — the Shop's scrollbar: a 14px outlined pill drawn beside a scroller whose own scrollbar is
// hidden, down its side (`axis` y, the default) or along its bottom (x). It follows the scroller by writing
// its own style on scroll and resize (no React render), and hides while everything fits. A drag of the pill
// scrolls, and a press on the track beside it brings the pill under the pointer first. Place it with the
// parent's layout; it fills the parent's height (or width, along x) as track.

import { useEffect, useRef } from 'react'
import styles from './ScrollThumb.module.css'

const MIN_THUMB = 40

export function ScrollThumb({
  target,
  length,
  axis = 'y',
  tone = 'outline',
  className
}: {
  target: React.RefObject<HTMLElement | null>
  /** A fixed thumb length (px), as a design draws it; by default it shows how much of the content is in view. */
  length?: number
  /** Down the scroller's side (y) or along its bottom (x). */
  axis?: 'x' | 'y'
  /** `dark` fills the pill, for a scroller over a light or busy surface. */
  tone?: 'outline' | 'dark'
  className?: string
}): React.JSX.Element {
  const track = useRef<HTMLSpanElement>(null)
  const thumb = useRef<HTMLSpanElement>(null)
  useEffect(() => {
    const el = target.current
    const tr = track.current
    const t = thumb.current
    if (el == null || tr == null || t == null) return
    const x = axis === 'x'
    // The thumb travels its own track, which may be shorter than the scroller (one that bleeds past
    // its box to leave room for a hovered card); the scroller only gives the proportions.
    const measure = (): { extent: number; free: number; range: number } | undefined => {
      const view = x ? el.clientWidth : el.clientHeight
      const size = x ? el.scrollWidth : el.scrollHeight
      if (size <= view + 1) return undefined
      const travel = x ? tr.clientWidth : tr.clientHeight
      const extent = Math.min(travel, length ?? Math.max(MIN_THUMB, (travel * view) / size))
      return { extent, free: travel - extent, range: size - view }
    }
    const offset = (): number => (x ? el.scrollLeft : el.scrollTop)
    const sync = (): void => {
      const m = measure()
      t.style.visibility = m == null ? 'hidden' : 'visible'
      tr.toggleAttribute('data-scrolls', m != null)
      if (m == null) return
      const { extent } = m
      const at = (m.free * offset()) / m.range
      if (x) {
        t.style.width = `${extent}px`
        t.style.transform = `translateX(${at}px)`
      } else {
        t.style.height = `${extent}px`
        t.style.transform = `translateY(${at}px)`
      }
    }
    sync()
    el.addEventListener('scroll', sync, { passive: true })
    const ro = new ResizeObserver(sync)
    ro.observe(el)
    ro.observe(tr)
    // The scroller's own box keeps its size when its content is swapped (loading → grid → empty), so each
    // child that comes in is watched too, and the thumb follows at once.
    for (const child of Array.from(el.children)) ro.observe(child)
    const mo = new MutationObserver((changes) => {
      for (const change of changes) {
        change.removedNodes.forEach((n) => n instanceof Element && ro.unobserve(n))
        change.addedNodes.forEach((n) => n instanceof Element && ro.observe(n))
      }
      sync()
    })
    mo.observe(el, { childList: true })

    // `from` is where the pointer went down and `at` where the pill was then, both along the track.
    let drag: { pointer: number; from: number; at: number } | undefined
    // A pointer moves in screen pixels and the track is measured in layout pixels; under `--ui-scale` they differ.
    const along = (e: PointerEvent): number => {
      const rect = tr.getBoundingClientRect()
      const laid = x ? tr.offsetWidth : tr.offsetHeight
      const drawn = x ? rect.width : rect.height
      return (x ? e.clientX - rect.left : e.clientY - rect.top) * (drawn > 0 && laid > 0 ? laid / drawn : 1)
    }
    const scrollTo = (thumbAt: number, m: { free: number; range: number }): void => {
      const to = Math.min(m.range, Math.max(0, (thumbAt * m.range) / m.free))
      if (x) el.scrollLeft = to
      else el.scrollTop = to
    }
    const down = (e: PointerEvent): void => {
      const m = measure()
      if (e.button !== 0 || m == null || m.free <= 0) return
      e.preventDefault()
      if (e.target !== t) scrollTo(along(e) - m.extent / 2, m)
      drag = { pointer: e.pointerId, from: along(e), at: (m.free * offset()) / m.range }
      tr.setPointerCapture?.(e.pointerId)
    }
    const move = (e: PointerEvent): void => {
      const m = measure()
      if (drag?.pointer !== e.pointerId || m == null || m.free <= 0) return
      scrollTo(drag.at + along(e) - drag.from, m)
    }
    const up = (e: PointerEvent): void => {
      if (drag?.pointer === e.pointerId) drag = undefined
    }
    tr.addEventListener('pointerdown', down)
    tr.addEventListener('pointermove', move)
    tr.addEventListener('pointerup', up)
    tr.addEventListener('pointercancel', up)
    return () => {
      tr.removeEventListener('pointerdown', down)
      tr.removeEventListener('pointermove', move)
      tr.removeEventListener('pointerup', up)
      tr.removeEventListener('pointercancel', up)
      el.removeEventListener('scroll', sync)
      ro.disconnect()
      mo.disconnect()
    }
  }, [target, length, axis])
  return (
    <span ref={track} className={`${styles.track} ${axis === 'x' ? styles.x : ''} ${tone === 'dark' ? styles.dark : ''} ${className ?? ''}`.trim()} aria-hidden="true">
      <span ref={thumb} className={styles.thumb} />
    </span>
  )
}
