import { useEffect, type RefObject } from 'react'

export type ScrollAxis = 'x' | 'y'

/**
 * Marks a scrolling element with `data-fade-start` / `data-fade-end` while content is hidden past that
 * edge, along the given axis. Written straight to the DOM (no React state): it runs on every scroll and
 * resize, and the styling it drives never changes the element's size, so nothing can loop.
 */
export function useScrollFades(ref: RefObject<HTMLElement>, axis: ScrollAxis = 'x'): void {
  useEffect(() => {
    const element = ref.current
    if (!element) return
    const update = () => {
      const scrolled = axis === 'x' ? element.scrollLeft : element.scrollTop
      const max = axis === 'x' ? element.scrollWidth - element.clientWidth : element.scrollHeight - element.clientHeight
      element.toggleAttribute('data-fade-start', scrolled > 1)
      element.toggleAttribute('data-fade-end', scrolled < max - 1)
    }
    update()
    element.addEventListener('scroll', update, { passive: true })
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(update)
    observer?.observe(element)
    return () => {
      element.removeEventListener('scroll', update)
      observer?.disconnect()
    }
  }, [ref, axis])
}
