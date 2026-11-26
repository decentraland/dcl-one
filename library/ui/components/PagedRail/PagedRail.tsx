import {
  Children,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode
} from 'react'
import { GeneralLeftArrow, GeneralRightArrow } from '../../foundations/icons/general'
import styles from './PagedRail.module.css'

export type PagedRailProps = {
  /** How many cards a page holds. */
  perPage: number
  /** The space between cards, in pixels. */
  gap: number
  /** `shop` has larger page dots, set further below the cards. */
  variant?: 'default' | 'shop'
  /**
   * Room in pixels inside the clip above, below and left of the cards, for a card that grows on hover;
   * the layout does not move. None to the right, where the clip is the edge the design draws.
   */
  bleed?: number
  /** Room above the cards when they rise further than they spread. `bleed` by default. */
  bleedTop?: number
  /** Page dots under the cards. Without them the arrows stay. */
  dots?: boolean
  /** The previous arrow's 15px mark; it takes the arrow's colour. A left arrow by default. */
  previousIcon?: ReactNode
  /** The next arrow's 15px mark. A right arrow by default. */
  nextIcon?: ReactNode
  /** The previous arrow's accessible name. "Previous" by default. */
  previousLabel?: string
  /** The next arrow's accessible name. "Next" by default. */
  nextLabel?: string
  /** A dot's accessible name, from its page counted from one. "Page N" by default. */
  pageLabel?: (page: number) => string
  /** The cards, one element each. */
  children: ReactNode
  className?: string
}

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
function measuredPages(
  el: HTMLDivElement,
  perPage: number,
  gap: number,
  bleed: number,
  fallback: number
): number {
  if (el.clientWidth === 0) return fallback
  const travel = cardsTravel(el, bleed)
  if (travel <= 1) return 1
  const stride = pageStride(el, perPage, gap)
  return stride > 0 ? 1 + Math.ceil((travel - 1) / stride) : fallback
}

const pageName = (page: number): string => `Page ${page}`

/**
 * A row of cards paged a whole page at a time: previous and next arrows that show while the row is
 * hovered or holds focus, and a dot per page below. The pages are what the viewport can scroll through,
 * measured again when it or its cards resize: when every card fits there is one page, and neither the
 * arrows nor the dots show.
 */
export function PagedRail({
  perPage,
  gap,
  variant = 'default',
  bleed = 0,
  bleedTop,
  dots = true,
  previousIcon,
  nextIcon,
  previousLabel = 'Previous',
  nextLabel = 'Next',
  pageLabel = pageName,
  children,
  className
}: PagedRailProps) {
  const viewport = useRef<HTMLDivElement>(null)
  const count = Children.count(children)
  const countPages = Math.max(1, Math.ceil(count / perPage))
  const [pages, setPages] = useState(countPages)
  const [page, setPage] = useState(0)

  useLayoutEffect(() => {
    const el = viewport.current
    if (el == null) return
    const measure = (): void => {
      const next = measuredPages(el, perPage, gap, bleed, countPages)
      setPages(next)
      setPage((current) => Math.min(current, next - 1))
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
    (target: number) => {
      const el = viewport.current
      const next = Math.max(0, Math.min(pages - 1, target))
      if (el == null) return
      const end = el.scrollWidth - el.clientWidth
      // A visitor who asks for less motion gets the page at once.
      const reduce = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches
      // The last page scrolls to the very end, so its last card keeps the end bleed's room on the right.
      el.scrollTo({
        left: next === pages - 1 ? end : Math.min(next * pageStride(el, perPage, gap), end),
        behavior: reduce ? 'auto' : 'smooth'
      })
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
    <div
      className={[
        styles.rail,
        variant === 'shop' && styles.shop,
        !dots && styles.noDots,
        className
      ]
        .filter(Boolean)
        .join(' ')}
    >
      <div
        ref={viewport}
        className={styles.viewport}
        style={
          bleed > 0 || bleedTop != null
            ? ({
                '--rail-bleed': `${bleed}px`,
                '--rail-bleed-top': `${bleedTop ?? bleed}px`
              } as CSSProperties)
            : undefined
        }
      >
        <div className={styles.track} style={{ gap }}>
          {children}
        </div>
      </div>
      {pages > 1 && (
        <>
          <div className={styles.arrows}>
            <button
              type="button"
              className={styles.arrow}
              aria-label={previousLabel}
              hidden={page === 0}
              onClick={() => goTo(page - 1)}
            >
              {previousIcon ?? <GeneralLeftArrow size={15} aria-hidden="true" />}
            </button>
            <button
              type="button"
              className={`${styles.arrow} ${styles.next}`}
              aria-label={nextLabel}
              hidden={page === pages - 1}
              onClick={() => goTo(page + 1)}
            >
              {nextIcon ?? <GeneralRightArrow size={15} aria-hidden="true" />}
            </button>
          </div>
          {dots && (
            <div className={styles.dots}>
              {Array.from({ length: pages }, (_, index) => (
                <button
                  key={index}
                  type="button"
                  aria-current={index === page ? 'true' : undefined}
                  aria-label={pageLabel(index + 1)}
                  className={[styles.dot, index === page && styles.dotActive]
                    .filter(Boolean)
                    .join(' ')}
                  onClick={() => goTo(index)}
                />
              ))}
            </div>
          )}
        </>
      )}
    </div>
  )
}
