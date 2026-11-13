// ScrollThumb — the Shop's scrollbar: a 14px outlined pill drawn beside a scroller whose own scrollbar is
// hidden, down its side (`axis` y, the default) or along its bottom (x). It follows the scroller by writing
// its own style on scroll and resize (no React render), and hides while everything fits. Place it with the
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
    const sync = (): void => {
      // The thumb travels its own track, which may be shorter than the scroller (one that bleeds past
      // its box to leave room for a hovered card); the scroller only gives the proportions.
      const view = x ? el.clientWidth : el.clientHeight
      const size = x ? el.scrollWidth : el.scrollHeight
      const offset = x ? el.scrollLeft : el.scrollTop
      const fits = size <= view + 1
      t.style.visibility = fits ? 'hidden' : 'visible'
      if (fits) return
      const travel = x ? tr.clientWidth : tr.clientHeight
      const extent = Math.min(travel, length ?? Math.max(MIN_THUMB, (travel * view) / size))
      const at = ((travel - extent) * offset) / (size - view)
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
    return () => {
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
