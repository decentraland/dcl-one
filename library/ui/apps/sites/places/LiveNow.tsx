import { useCallback, useEffect, useId, useRef, useState, type KeyboardEvent } from 'react'
import { GeneralLeftArrow, GeneralLive, GeneralRightArrow } from '../../../foundations/icons'
import { Button } from '../shared/Button/Button'
import { LiveEventCardSlot } from './cards/LiveEventCardSlot'
import type { PlaceCardData, PlaceCardHandlers, PlaceCardLabels } from './cards/types'
import styles from './LiveNow.module.css'

/** Sub-pixel slack, so a rail scrolled to its end does not keep claiming there is more. */
const EDGE_TOLERANCE_PX = 2
const REDUCED_MOTION = '(prefers-reduced-motion: reduce)'

type LiveNowLabels = {
  previous: string
  next: string
  /** Names a dot after the card it scrolls to. */
  goTo: (index: number, title: string) => string
}

type LiveNowProps = {
  title: string
  cards: readonly PlaceCardData[]
  labels: LiveNowLabels
  cardLabels: PlaceCardLabels
} & PlaceCardHandlers

function scrollBehavior(): ScrollBehavior {
  return window.matchMedia?.(REDUCED_MOTION).matches ? 'auto' : 'smooth'
}

/**
 * Whether a horizontally overflowing rail has content left on either side, and which card is
 * centred. Re-measured on scroll, resize and when the card count changes (a rail that grows from
 * three cards to four grows its scroll width while its own box keeps its size).
 */
function useRailEdges(count: number) {
  const rail = useRef<HTMLUListElement>(null)
  const [edges, setEdges] = useState({ left: false, right: false })
  const [active, setActive] = useState(0)

  const measure = useCallback(() => {
    const el = rail.current
    if (!el) return
    const { clientWidth, scrollLeft, scrollWidth } = el
    setEdges({
      left: scrollLeft > EDGE_TOLERANCE_PX,
      right: scrollLeft + clientWidth < scrollWidth - EDGE_TOLERANCE_PX
    })
    const center = scrollLeft + clientWidth / 2
    let nearest = 0
    let distance = Infinity
    Array.from(el.children).forEach((child, index) => {
      const slide = child as HTMLElement
      const gap = Math.abs(slide.offsetLeft + slide.offsetWidth / 2 - center)
      if (gap < distance) {
        distance = gap
        nearest = index
      }
    })
    setActive(nearest)
  }, [])

  useEffect(() => {
    const el = rail.current
    if (!el) return
    measure()
    el.addEventListener('scroll', measure, { passive: true })
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(measure)
    observer?.observe(el)
    return () => {
      el.removeEventListener('scroll', measure)
      observer?.disconnect()
    }
  }, [measure, count])

  return { rail, edges, active: Math.min(active, Math.max(count - 1, 0)) }
}

/**
 * Source's Live Now section: a glowing panel with the busiest scenes. TODO(figma-ds): the shared
 * Carousel shows one centred slide, while this rail is a row of up to four on desktop (arrows while
 * it overflows) and one card per swipe on phones (dots); it moves to the design system's
 * multi-item carousel once there is one.
 */
function LiveNow({ title, cards, labels, cardLabels, onOpen, onJumpIn }: LiveNowProps) {
  const titleId = useId()
  const { rail, edges, active } = useRailEdges(cards.length)

  const nudge = (direction: -1 | 1) => {
    const el = rail.current
    if (!el || !el.children.length) return
    // One card per step: the average span includes the gap.
    el.scrollBy?.({
      left: (direction * el.scrollWidth) / el.children.length,
      behavior: scrollBehavior()
    })
  }
  const goTo = (index: number) => {
    const slide = rail.current?.children[index] as HTMLElement | undefined
    slide?.scrollIntoView?.({ behavior: scrollBehavior(), inline: 'center', block: 'nearest' })
  }
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return
    event.preventDefault()
    nudge(event.key === 'ArrowLeft' ? -1 : 1)
  }

  if (!cards.length) return null
  return (
    <section className={styles.root} aria-labelledby={titleId}>
      <h2 id={titleId} className={styles.heading}>
        <GeneralLive className={styles.headingIcon} aria-hidden="true" />
        {title}
      </h2>
      <div
        className={styles.layer}
        role="group"
        aria-roledescription="carousel"
        aria-labelledby={titleId}
        onKeyDown={onKeyDown}
      >
        {/* Both stay mounted and go disabled at the ends, so the pressed one keeps the focus. */}
        {edges.left || edges.right ? (
          <>
            <div className={`${styles.arrow} ${styles.arrowStart}`}>
              <Button
                variant="quaternary"
                aria-label={labels.previous}
                disabled={!edges.left}
                onClick={() => nudge(-1)}
              >
                <GeneralLeftArrow size={24} aria-hidden="true" />
              </Button>
            </div>
            <div className={`${styles.arrow} ${styles.arrowEnd}`}>
              <Button
                variant="quaternary"
                aria-label={labels.next}
                disabled={!edges.right}
                onClick={() => nudge(1)}
              >
                <GeneralRightArrow size={24} aria-hidden="true" />
              </Button>
            </div>
          </>
        ) : null}
        <ul ref={rail} className={styles.rail}>
          {cards.map((card) => (
            <li key={card.id} className={styles.slide}>
              <LiveEventCardSlot
                card={card}
                labels={cardLabels}
                onOpen={onOpen}
                onJumpIn={onJumpIn}
              />
            </li>
          ))}
        </ul>
      </div>
      {cards.length > 1 ? (
        <div className={styles.dots}>
          {cards.map((card, index) => (
            <button
              key={card.id}
              type="button"
              className={index === active ? `${styles.dot} ${styles.dotActive}` : styles.dot}
              aria-label={labels.goTo(index + 1, card.title)}
              aria-current={index === active ? 'true' : undefined}
              onClick={() => goTo(index)}
            />
          ))}
        </div>
      ) : null}
    </section>
  )
}

export { LiveNow }
export type { LiveNowLabels, LiveNowProps }
