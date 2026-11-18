import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent,
  type ReactNode,
  type TouchEvent,
  type TransitionEvent
} from 'react'
import { useMediaQuery } from '~/hooks/useMediaQuery'
import { useTranslation } from '~/intl'
import { media } from '~/styles/theme'
import styles from './Carousel.module.css'

const SWIPE_THRESHOLD = 50
const SLIDE_MS = 400
const DESKTOP_GAP = 24
const TABLET_GAP = 16
/** Phones show one slide with a 16px gutter each side. */
const PHONE_GUTTER = 32
/** Below this a slide fills the viewport; sites' phone cut for the carousel, narrower than the app's mobile breakpoint. */
const PHONE = '@media (max-width: 599px)'
/** Without a way to measure the wrapper the track shows unmeasured rather than never. */
const CAN_MEASURE = typeof ResizeObserver !== 'undefined'

type CarouselProps<T> = {
  items: T[]
  /** `slide` is the rendered copy's index, unique across the clones: suffix any DOM id with it. */
  renderItem: (item: T, slide: number) => ReactNode
  keyExtractor: (item: T) => string
  /** Accessible name of the region. */
  label: string
  /** Desktop slide width in px; tablets show 70% of the viewport, phones the viewport minus the gutters. */
  slideWidth: number
  /** Advance automatically every N ms; 0 disables. Stops while the tab is hidden and restarts after any interaction. */
  autoplayMs?: number
  /** Vertical alignment of slides of different heights. */
  alignItems?: 'stretch' | 'center'
}

/**
 * Infinite carousel: the track is translated one slide at a time and the items are rendered three times
 * (clone, real, clone), so the slide next to either end is a real DOM node the transition can move to.
 * Landing on a clone snaps `pos` back into the middle copy with transitions off, which is invisible
 * because every neighbour matches its real counterpart. Drag (mouse and touch), arrows and dots all
 * move the same `pos`.
 */
function Carousel<T>({
  items,
  renderItem,
  keyExtractor,
  label,
  slideWidth: desktopSlideWidth,
  autoplayMs = 0,
  alignItems = 'stretch'
}: CarouselProps<T>) {
  const { t } = useTranslation()
  const desktop = useMediaQuery(media.minWidth('tablet'))
  const phone = useMediaQuery(PHONE)
  const total = items.length
  const slides = useMemo(() => [...items, ...items, ...items], [items])

  // Index into `slides`; the real items sit at total..2*total-1.
  const [pos, setPos] = useState(total)
  const [animated, setAnimated] = useState(true)
  const [dragOffset, setDragOffset] = useState(0)
  const [viewportWidth, setViewportWidth] = useState(0)

  const wrapperRef = useRef<HTMLDivElement>(null)
  const autoRef = useRef<ReturnType<typeof setInterval>>()
  const settleRef = useRef<ReturnType<typeof setTimeout>>()
  const movingRef = useRef(false)
  const dragStartXRef = useRef(0)
  const dragOffsetRef = useRef(0)
  const draggingRef = useRef(false)
  const wasDragRef = useRef(false)

  useEffect(() => {
    const el = wrapperRef.current
    if (!el || !CAN_MEASURE) return
    const observer = new ResizeObserver(([entry]) => setViewportWidth(entry.contentRect.width))
    observer.observe(el)
    return () => observer.disconnect()
  }, [])

  const slideWidth = desktop ? desktopSlideWidth : phone ? viewportWidth - PHONE_GUTTER : viewportWidth * 0.7
  const gap = desktop ? DESKTOP_GAP : phone ? PHONE_GUTTER : TABLET_GAP
  const step = slideWidth + gap
  const translateX = viewportWidth / 2 - slideWidth / 2 - pos * step + dragOffset
  const realIndex = ((pos % total) + total) % total

  // Landing on a clone: jump to the same item in the middle copy without animating.
  const settle = useCallback(() => {
    clearTimeout(settleRef.current)
    movingRef.current = false
    setPos(p => {
      if (p >= 2 * total) {
        setAnimated(false)
        return p - total
      }
      if (p < total) {
        setAnimated(false)
        return p + total
      }
      return p
    })
  }, [total])

  // Blocks further moves until the slide animation ends; the timer covers a transitionend that never
  // arrives (hidden tab, transitions disabled by the user agent).
  const beginMove = useCallback(() => {
    movingRef.current = true
    setAnimated(true)
    clearTimeout(settleRef.current)
    settleRef.current = setTimeout(settle, SLIDE_MS + 100)
  }, [settle])

  useEffect(() => () => clearTimeout(settleRef.current), [])

  const move = useCallback(
    (delta: number) => {
      if (movingRef.current) return
      beginMove()
      setPos(p => p + delta)
    },
    [beginMove]
  )

  const startAutoplay = useCallback(() => {
    clearInterval(autoRef.current)
    if (!autoplayMs) return
    autoRef.current = setInterval(() => move(1), autoplayMs)
  }, [autoplayMs, move])

  const goTo = useCallback(
    (index: number) => {
      if (movingRef.current || index === realIndex) return
      beginMove()
      setPos(total + index)
      startAutoplay()
    },
    [realIndex, total, beginMove, startAutoplay]
  )

  const onTransitionEnd = useCallback(
    (event: TransitionEvent) => {
      if (event.target === event.currentTarget) settle()
    },
    [settle]
  )

  // Transitions come back the frame after the snap has painted.
  useEffect(() => {
    if (animated) return
    const id = requestAnimationFrame(() => requestAnimationFrame(() => setAnimated(true)))
    return () => cancelAnimationFrame(id)
  }, [animated])

  useEffect(() => {
    startAutoplay()
    return () => clearInterval(autoRef.current)
  }, [startAutoplay])

  useEffect(() => {
    const onVisibility = () => (document.hidden ? clearInterval(autoRef.current) : startAutoplay())
    document.addEventListener('visibilitychange', onVisibility)
    return () => document.removeEventListener('visibilitychange', onVisibility)
  }, [startAutoplay])

  const endDrag = useCallback(() => {
    draggingRef.current = false
    const dx = dragOffsetRef.current
    dragOffsetRef.current = 0
    setDragOffset(0)
    setAnimated(true)
    if (dx < -SWIPE_THRESHOLD) {
      beginMove()
      setPos(p => p + 1)
    } else if (dx > SWIPE_THRESHOLD) {
      beginMove()
      setPos(p => p - 1)
    }
    startAutoplay()
  }, [beginMove, startAutoplay])

  const beginDrag = useCallback((clientX: number) => {
    dragStartXRef.current = clientX
    dragOffsetRef.current = 0
    draggingRef.current = true
    wasDragRef.current = false
    setAnimated(false)
    clearInterval(autoRef.current)
  }, [])

  const dragTo = useCallback((clientX: number) => {
    if (!draggingRef.current) return
    const dx = clientX - dragStartXRef.current
    dragOffsetRef.current = dx
    setDragOffset(dx)
    if (Math.abs(dx) > 5) wasDragRef.current = true
  }, [])

  const onTouchStart = useCallback((event: TouchEvent) => beginDrag(event.touches[0].clientX), [beginDrag])
  const onTouchMove = useCallback((event: TouchEvent) => dragTo(event.touches[0].clientX), [dragTo])
  const onTouchEnd = useCallback(() => {
    if (draggingRef.current) endDrag()
  }, [endDrag])

  const onMouseDown = useCallback(
    (event: MouseEvent) => {
      if (event.button !== 0) return
      event.preventDefault()
      beginDrag(event.clientX)
    },
    [beginDrag]
  )

  // The pointer may leave the track mid-drag, so moves and the release are tracked on the window.
  useEffect(() => {
    const onMouseMove = (event: globalThis.MouseEvent) => dragTo(event.clientX)
    const onMouseUp = () => {
      if (draggingRef.current) endDrag()
    }
    window.addEventListener('mousemove', onMouseMove)
    window.addEventListener('mouseup', onMouseUp)
    return () => {
      window.removeEventListener('mousemove', onMouseMove)
      window.removeEventListener('mouseup', onMouseUp)
    }
  }, [dragTo, endDrag])

  // A drag that ends over a link must not open it.
  const onClickCapture = useCallback((event: MouseEvent) => {
    if (!wasDragRef.current) return
    event.preventDefault()
    event.stopPropagation()
    wasDragRef.current = false
  }, [])

  const position = (i: number) => (i === pos ? 'active' : i === pos - 1 ? 'prev' : i === pos + 1 ? 'next' : undefined)

  return (
    <div
      className={styles.wrapper}
      ref={wrapperRef}
      role="region"
      aria-roledescription="carousel"
      aria-label={label}
      data-testid="carousel"
    >
      <div
        className={styles.track}
        data-testid="carousel-track"
        style={{
          transform: `translateX(${translateX}px)`,
          transition: animated ? `transform ${SLIDE_MS}ms ease` : 'none',
          gap,
          alignItems,
          visibility: viewportWidth > 0 || !CAN_MEASURE ? 'visible' : 'hidden'
        }}
        onMouseDown={onMouseDown}
        onClickCapture={onClickCapture}
        onTouchStart={onTouchStart}
        onTouchMove={onTouchMove}
        onTouchEnd={onTouchEnd}
        onTransitionEnd={onTransitionEnd}
      >
        {slides.map((item, i) => (
          <div
            className={styles.slide}
            key={`${keyExtractor(item)}-${i}`}
            data-testid="carousel-slide"
            data-position={position(i)}
            data-active={i === pos || undefined}
            aria-hidden={i !== pos || undefined}
            // aria-hidden alone leaves the clones' links and tabs in the tab order.
            inert={i !== pos ? '' : undefined}
            style={{ width: slideWidth || undefined, transition: animated ? undefined : 'none' }}
          >
            {renderItem(item, i)}
          </div>
        ))}
      </div>
      <div className={styles.dots} data-testid="carousel-dots">
        {items.map((item, i) => (
          <button
            className={styles.dot}
            key={keyExtractor(item)}
            type="button"
            data-active={i === realIndex || undefined}
            aria-current={i === realIndex || undefined}
            aria-label={t('overview.carousel.go_to', { index: i + 1 })}
            onClick={() => goTo(i)}
          />
        ))}
      </div>
      <button
        className={styles.arrow}
        type="button"
        data-direction="prev"
        aria-label={t('overview.carousel.previous')}
        onClick={() => {
          move(-1)
          startAutoplay()
        }}
      >
        &#8249;
      </button>
      <button
        className={styles.arrow}
        type="button"
        data-direction="next"
        aria-label={t('overview.carousel.next')}
        onClick={() => {
          move(1)
          startAutoplay()
        }}
      >
        &#8250;
      </button>
    </div>
  )
}

export { Carousel }
