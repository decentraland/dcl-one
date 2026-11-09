// Rail — a horizontal carousel paged a whole page at a time: dots below (one per page, none for a
// single page) and prev/next arrows that fade in while the section is hovered. The pages are what the
// viewport can actually scroll through: when every card fits there is one, so neither shows.

import { Children, useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'
import arrowLeft from '../assets/rail/arrow-left.png'
import arrowRight from '../assets/rail/arrow-right.png'
import { MaskIcon } from './MaskIcon'
import styles from './Rail.module.css'

function pageStride(el: HTMLDivElement, perPage: number, gap: number): number {
  const card = el.firstElementChild?.firstElementChild as HTMLElement | null
  return perPage * ((card?.offsetWidth ?? 0) + gap)
}

/** How far the cards themselves scroll: the viewport's whole scroll less the end `bleed`, which is padding. */
function cardsTravel(el: HTMLDivElement, bleed: number): number {
  return Math.max(0, el.scrollWidth - el.clientWidth - bleed)
}

/**
 * The pages the cards scroll through: one per stride, the last clamped to the end of the cards, and one
 * when they fit. Before the viewport has a width (no layout yet), `fallback`.
 */
function measuredPages(el: HTMLDivElement, perPage: number, gap: number, bleed: number, fallback: number): number {
  if (el.clientWidth === 0) return fallback
  const travel = cardsTravel(el, bleed)
  if (travel <= 1) return 1
  const stride = pageStride(el, perPage, gap)
  return stride > 0 ? 1 + Math.ceil((travel - 1) / stride) : fallback
}

export function Rail({
  perPage,
  gap,
  variant = 'default',
  bleed = 0,
  bleedTop,
  dots = true,
  children,
  className
}: {
  perPage: number
  /** Space between cards, px. */
  gap: number
  /** `shop`: the Shop home's larger page dots, set further below the cards. */
  variant?: 'default' | 'shop'
  /** Room (px) inside the clip above, below and left of the cards, for a card that grows on hover; the
   *  layout doesn't move. None to the right, where the clip is the edge the design draws. */
  bleed?: number
  /** Room above the cards when they rise further than they spread (art drawn over their top edge);
   *  `bleed` by default. */
  bleedTop?: number
  /** Page dots under the cards (the arrows stay). */
  dots?: boolean
  children: React.ReactNode
  className?: string
}): React.JSX.Element {
  const viewport = useRef<HTMLDivElement>(null)
  const count = Children.count(children)
  const countPages = Math.max(1, Math.ceil(count / perPage))
  const [pages, setPages] = useState(countPages)
  const [page, setPage] = useState(0)

  // Measured on every resize of the viewport or its cards, so a wider window drops the pages it no longer needs.
  useLayoutEffect(() => {
    const el = viewport.current
    if (el == null) return
    const measure = (): void => {
      const next = measuredPages(el, perPage, gap, bleed, countPages)
      setPages(next)
      setPage((p) => Math.min(p, next - 1))
    }
    measure()
    if (typeof ResizeObserver === 'undefined') {
      window.addEventListener('resize', measure)
      return () => window.removeEventListener('resize', measure)
    }
    const observer = new ResizeObserver(measure)
    observer.observe(el)
    if (el.firstElementChild != null) observer.observe(el.firstElementChild)
    return () => observer.disconnect()
  }, [bleed, count, countPages, gap, perPage])

  const goTo = useCallback(
    (p: number) => {
      const el = viewport.current
      const next = Math.max(0, Math.min(pages - 1, p))
      if (el == null) return
      const end = el.scrollWidth - el.clientWidth
      // The last page scrolls to the very end, so its last card keeps the end bleed's room on the right.
      el.scrollTo({ left: next === pages - 1 ? end : Math.min(next * pageStride(el, perPage, gap), end), behavior: 'smooth' })
    },
    [gap, pages, perPage]
  )

  useEffect(() => {
    const el = viewport.current
    if (el == null) return
    const onScroll = (): void => {
      const travel = cardsTravel(el, bleed)
      const stride = pageStride(el, perPage, gap)
      if (travel <= 0 || stride <= 0) setPage(0)
      // The end of the cards, or past it into the end bleed, is the last page.
      else if (el.scrollLeft >= travel - 1) setPage(pages - 1)
      else setPage(Math.max(0, Math.min(pages - 2, Math.round(el.scrollLeft / stride))))
    }
    el.addEventListener('scroll', onScroll, { passive: true })
    return () => el.removeEventListener('scroll', onScroll)
  }, [bleed, gap, pages, perPage])

  useEffect(() => {
    viewport.current?.scrollTo({ left: 0 })
    setPage(0)
  }, [count])

  return (
    <div className={`${styles.rail} ${variant === 'shop' ? styles.shop : ''} ${dots ? '' : styles.noDots} ${className ?? ''}`.replace(/\s+/g, ' ').trim()}>
      <div
        ref={viewport}
        className={styles.viewport}
        style={bleed > 0 || bleedTop != null ? ({ '--rail-bleed': `${bleed}px`, '--rail-bleed-top': `${bleedTop ?? bleed}px` } as React.CSSProperties) : undefined}
      >
        <div className={styles.track} style={{ gap }}>
          {children}
        </div>
      </div>
      {pages > 1 && (
        <>
          <div className={styles.arrows}>
            <button type="button" className={styles.arrow} aria-label="Previous" hidden={page === 0} onClick={() => goTo(page - 1)}>
              <MaskIcon src={arrowLeft} size={15} className={styles.arrowIcon} />
            </button>
            <button type="button" className={`${styles.arrow} ${styles.next}`} aria-label="Next" hidden={page === pages - 1} onClick={() => goTo(page + 1)}>
              <MaskIcon src={arrowRight} size={15} className={styles.arrowIcon} />
            </button>
          </div>
          {dots && (
            <div className={styles.dots}>
              {Array.from({ length: pages }, (_, i) => (
                <button
                  key={i}
                  type="button"
                  aria-current={i === page ? 'true' : undefined}
                  aria-label={`Page ${i + 1}`}
                  className={`${styles.dot} ${i === page ? styles.dotActive : ''}`.trim()}
                  onClick={() => goTo(i)}
                />
              ))}
            </div>
          )}
        </>
      )}
    </div>
  )
}
