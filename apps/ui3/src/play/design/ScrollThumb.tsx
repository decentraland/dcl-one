// ScrollThumb — the Shop's scrollbar: a 14px outlined pill drawn beside a scroller whose own scrollbar is
// hidden. It follows the scroller by writing its own style on scroll and resize (no React render), and
// hides while everything fits. Place it with the parent's layout; it fills the parent's height as track.

import { useEffect, useRef } from 'react'
import styles from './ScrollThumb.module.css'

const MIN_THUMB = 40

export function ScrollThumb({ target, className }: { target: React.RefObject<HTMLElement | null>; className?: string }): React.JSX.Element {
  const thumb = useRef<HTMLSpanElement>(null)
  useEffect(() => {
    const el = target.current
    const t = thumb.current
    if (el == null || t == null) return
    const sync = (): void => {
      const track = el.clientHeight
      const fits = el.scrollHeight <= track + 1
      t.style.visibility = fits ? 'hidden' : 'visible'
      if (fits) return
      const height = Math.max(MIN_THUMB, (track * track) / el.scrollHeight)
      const top = ((track - height) * el.scrollTop) / (el.scrollHeight - track)
      t.style.height = `${height}px`
      t.style.transform = `translateY(${top}px)`
    }
    sync()
    el.addEventListener('scroll', sync, { passive: true })
    const ro = new ResizeObserver(sync)
    ro.observe(el)
    for (const child of Array.from(el.children)) ro.observe(child)
    return () => {
      el.removeEventListener('scroll', sync)
      ro.disconnect()
    }
  }, [target])
  return (
    <span className={`${styles.track} ${className ?? ''}`.trim()} aria-hidden="true">
      <span ref={thumb} className={styles.thumb} />
    </span>
  )
}
