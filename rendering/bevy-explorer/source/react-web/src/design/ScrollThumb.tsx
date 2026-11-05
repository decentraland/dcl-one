// ScrollThumb — the Shop's scrollbar: a 14px outlined pill drawn beside a scroller whose own scrollbar is
// hidden. It follows the scroller by writing its own style on scroll and resize (no React render), and
// hides while everything fits. Place it with the parent's layout; it fills the parent's height as track.

import { useEffect, useRef } from 'react'
import styles from './ScrollThumb.module.css'

const MIN_THUMB = 40

export function ScrollThumb({
  target,
  length,
  className
}: {
  target: React.RefObject<HTMLElement | null>
  /** A fixed thumb length (px), as a design draws it; by default it shows how much of the content is in view. */
  length?: number
  className?: string
}): React.JSX.Element {
  const track = useRef<HTMLSpanElement>(null)
  const thumb = useRef<HTMLSpanElement>(null)
  useEffect(() => {
    const el = target.current
    const tr = track.current
    const t = thumb.current
    if (el == null || tr == null || t == null) return
    const sync = (): void => {
      // The thumb travels its own track, which may be shorter than the scroller (one that bleeds past
      // its box to leave room for a hovered card); the scroller only gives the proportions.
      const view = el.clientHeight
      const fits = el.scrollHeight <= view + 1
      t.style.visibility = fits ? 'hidden' : 'visible'
      if (fits) return
      const travel = tr.clientHeight
      const height = Math.min(travel, length ?? Math.max(MIN_THUMB, (travel * view) / el.scrollHeight))
      const top = ((travel - height) * el.scrollTop) / (el.scrollHeight - view)
      t.style.height = `${height}px`
      t.style.transform = `translateY(${top}px)`
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
  }, [target, length])
  return (
    <span ref={track} className={`${styles.track} ${className ?? ''}`.trim()} aria-hidden="true">
      <span ref={thumb} className={styles.thumb} />
    </span>
  )
}
