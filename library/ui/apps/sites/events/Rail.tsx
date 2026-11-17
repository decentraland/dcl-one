import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react'
import { GeneralLeftArrow, GeneralRightArrow } from '../../../foundations/icons'
import styles from './Rail.module.css'

// TODO(figma-ds): a minimal paged scroller. `../shared/Carousel` shows one looping slide at a time;
// these rails page through several cards, so they use native scrolling with scroll snap until the
// design system ships a multi-card carousel.

type Paging = { pages: number; active: number }

function measure(el: HTMLElement): Paging {
  const { clientWidth, scrollWidth, scrollLeft } = el
  if (clientWidth <= 0) return { pages: 1, active: 0 }
  // Each page scrolls one viewport; gaps and rounding leave a sliver past the last page that is not one.
  const pages = Math.max(1, Math.ceil((scrollWidth - clientWidth) / clientWidth - 0.25) + 1)
  const active = Math.min(pages - 1, Math.max(0, Math.round(scrollLeft / clientWidth)))
  return { pages, active }
}

/** The page count and the page in view of a horizontally scrolling element. */
function useScrollPages<T extends HTMLElement = HTMLDivElement>(count: number) {
  const ref = useRef<T | null>(null)
  const [paging, setPaging] = useState<Paging>({ pages: 1, active: 0 })
  const sync = useCallback(() => {
    const el = ref.current
    if (!el) return
    const next = measure(el)
    setPaging((prev) => (prev.pages === next.pages && prev.active === next.active ? prev : next))
  }, [])
  useEffect(() => {
    const el = ref.current
    if (!el) return
    sync()
    if (typeof ResizeObserver === 'undefined') return
    // The content's width moves the page count too, without resizing the track itself.
    const observer = new ResizeObserver(sync)
    observer.observe(el)
    for (const child of el.children) observer.observe(child)
    return () => observer.disconnect()
  }, [count, sync])
  const goTo = useCallback((page: number) => {
    const el = ref.current
    if (!el) return
    // A visitor who asks for less motion gets the page at once.
    const reduce = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches
    el.scrollTo({ left: page * el.clientWidth, behavior: reduce ? 'auto' : 'smooth' })
  }, [])
  return { ref, ...paging, sync, goTo }
}

/** One dot per page; the current one is marked for assistive technology. */
function PageDots({
  pages,
  active,
  label,
  onSelect
}: {
  pages: number
  active: number
  label: string
  onSelect: (page: number) => void
}) {
  if (pages <= 1) return null
  return (
    <div className={styles.dots} role="group" aria-label={`${label} pages`}>
      {Array.from({ length: pages }, (_, page) => (
        <button
          key={page}
          type="button"
          className={styles.dot}
          aria-label={`Page ${page + 1} of ${pages}`}
          aria-current={page === active ? 'true' : undefined}
          onClick={() => onSelect(page)}
        />
      ))}
    </div>
  )
}

/** A horizontal rail with previous and next controls from desktop and page dots below. */
function Rail({
  label,
  count,
  className,
  trackClassName,
  children
}: {
  label: string
  count: number
  className?: string
  trackClassName?: string
  children: ReactNode
}) {
  const { ref, pages, active, sync, goTo } = useScrollPages(count)
  return (
    <div className={className}>
      <div className={styles.layer}>
        {active > 0 ? (
          <button
            type="button"
            className={`${styles.chevron} ${styles.previous}`}
            aria-label={`Show previous ${label}`}
            onClick={() => goTo(active - 1)}
          >
            <GeneralLeftArrow size={24} aria-hidden="true" />
          </button>
        ) : null}
        <div ref={ref} className={`${styles.track} ${trackClassName ?? ''}`} onScroll={sync}>
          {children}
        </div>
        {active < pages - 1 ? (
          <button
            type="button"
            className={`${styles.chevron} ${styles.next}`}
            aria-label={`Show next ${label}`}
            onClick={() => goTo(active + 1)}
          >
            <GeneralRightArrow size={24} aria-hidden="true" />
          </button>
        ) : null}
      </div>
      <PageDots pages={pages} active={active} label={label} onSelect={goTo} />
    </div>
  )
}

export { PageDots, Rail, useScrollPages }
