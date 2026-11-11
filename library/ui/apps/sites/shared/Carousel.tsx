import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type MouseEvent,
  type PointerEvent,
  type ReactNode
} from 'react'
import { Button } from '../shared/Button/Button'
import { GeneralLeftArrow, GeneralRightArrow } from '../../../foundations/icons'
import styles from './Carousel.module.css'

type CarouselLabels = {
  previous: string
  next: string
  goTo: (slide: number) => string
  position: (slide: number, total: number) => string
}
type CarouselProps<T> = {
  items: readonly T[]
  getKey: (item: T) => string
  renderSlide: (item: T, peek: boolean) => ReactNode
  labelledBy: string
  labels: CarouselLabels
  className?: string
  slideClassName?: string
  /** Source's weekly carousel uses 4000ms; video clips stay manual. */
  autoplayDelay?: number
}
const SWIPE_THRESHOLD_PX = 50
const TRANSITION_MS = 400
const REDUCED_MOTION = '(prefers-reduced-motion: reduce)'

/** Adjacent copies let source's track slide through a wrap before an invisible reset. */
function Carousel<T>({
  items,
  getKey,
  renderSlide,
  labelledBy,
  labels,
  className,
  slideClassName,
  autoplayDelay = 0
}: CarouselProps<T>) {
  const total = items.length
  const [position, setPosition] = useState(total)
  const [animated, setAnimated] = useState(false)
  const [dragOffset, setDragOffset] = useState(0)
  const [reduced, setReduced] = useState(true)
  const [hovered, setHovered] = useState(false)
  const [focused, setFocused] = useState(false)
  const [dragging, setDragging] = useState(false)
  const [visible, setVisible] = useState(true)
  const [interaction, setInteraction] = useState(0)
  const current = total ? ((position % total) + total) % total : 0
  const paused = hovered || focused || dragging || !visible
  const activeSlide = useRef<HTMLDivElement | null>(null)
  const keepFocus = useRef<'slide' | 'control' | false>(false)
  const drag = useRef<{ x: number; y: number } | null>(null)
  const dragged = useRef(false)
  const clickReset = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)

  useEffect(() => {
    const preference = window.matchMedia?.(REDUCED_MOTION)
    const update = () => setReduced(preference?.matches ?? false)
    update()
    preference?.addEventListener('change', update)
    return () => preference?.removeEventListener('change', update)
  }, [])
  useEffect(() => {
    const update = () => setVisible(!document.hidden)
    update()
    document.addEventListener('visibilitychange', update)
    return () => document.removeEventListener('visibilitychange', update)
  }, [])
  useEffect(() => () => clearTimeout(clickReset.current), [])
  useEffect(() => {
    setAnimated(false)
    setPosition(total)
  }, [total])

  const move = useCallback(
    (offset: number, manual = true) => {
      if (!total) return
      if (manual) setInteraction((value) => value + 1)
      setAnimated(!reduced)
      setPosition((previous) => {
        const next = previous + offset
        if (reduced) return total + (((next % total) + total) % total)
        // Fast repeated input may exhaust the clone range before a transition settles.
        return next < 0 || next >= total * 3 ? total + (((next % total) + total) % total) : next
      })
    },
    [total, reduced]
  )
  const go = (index: number) => {
    setInteraction((value) => value + 1)
    setAnimated(!reduced)
    setPosition(total + index)
  }
  const rememberFocus = () => {
    const slide = activeSlide.current
    keepFocus.current = slide?.contains(document.activeElement)
      ? slide === document.activeElement
        ? 'slide'
        : 'control'
      : false
  }
  const settle = useCallback(() => {
    if (!total || (position >= total && position < total * 2)) return
    const slide = activeSlide.current
    keepFocus.current = slide?.contains(document.activeElement)
      ? slide === document.activeElement
        ? 'slide'
        : 'control'
      : false
    setAnimated(false)
    setPosition(total + current)
  }, [position, total, current])
  useEffect(() => {
    if (!animated) return
    // Also settles when transitionend is absent after a resize, interruption or hidden tab.
    const timeout = setTimeout(settle, reduced ? 0 : TRANSITION_MS)
    return () => clearTimeout(timeout)
  }, [animated, settle, reduced])
  useEffect(() => {
    if (!Number.isFinite(autoplayDelay) || autoplayDelay <= 0 || total < 2 || reduced || paused)
      return
    const interval = setInterval(() => move(1, false), autoplayDelay)
    return () => clearInterval(interval)
  }, [autoplayDelay, total, reduced, paused, move, interaction])
  useEffect(() => {
    const slide = activeSlide.current
    const target = keepFocus.current
    keepFocus.current = false
    if (!target || !slide || slide.contains(document.activeElement)) return
    const control =
      target === 'control'
        ? slide.querySelector<HTMLElement>(
            'a[href], button:not([disabled]), [tabindex]:not([tabindex="-1"])'
          )
        : null
    ;(control ?? slide).focus()
  }, [current, position])

  const cancelDrag = () => {
    drag.current = null
    dragged.current = false
    setDragging(false)
    setDragOffset(0)
  }
  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (
      event.button !== 0 ||
      (event.target instanceof Element &&
        event.target.closest(
          'button, input, select, textarea, [role="button"], [contenteditable="true"]'
        ))
    )
      return
    drag.current = { x: event.clientX, y: event.clientY }
    dragged.current = false
    if (animated) settle()
    else setAnimated(false)
    setDragging(true)
  }
  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const start = drag.current
    if (!start) return
    if (event.pointerType === 'mouse' && event.buttons === 0) {
      cancelDrag()
      return
    }
    const dx = event.clientX - start.x
    if (Math.abs(dx) <= Math.abs(event.clientY - start.y)) return
    setDragOffset(dx)
    if (Math.abs(dx) > 5) dragged.current = true
    if (
      event.pointerType === 'mouse' &&
      Math.abs(dx) >= SWIPE_THRESHOLD_PX &&
      !event.currentTarget.hasPointerCapture?.(event.pointerId)
    )
      event.currentTarget.setPointerCapture?.(event.pointerId)
  }
  const onPointerUp = (event: PointerEvent<HTMLDivElement>) => {
    const start = drag.current
    drag.current = null
    setDragging(false)
    setDragOffset(0)
    if (!start) return
    const dx = event.clientX - start.x
    const horizontal = Math.abs(dx) > Math.abs(event.clientY - start.y)
    if (horizontal && Math.abs(dx) >= SWIPE_THRESHOLD_PX) {
      dragged.current = true
      move(dx < 0 ? 1 : -1)
    }
    clearTimeout(clickReset.current)
    clickReset.current = setTimeout(() => (dragged.current = false), 0)
  }
  const onClickCapture = (event: MouseEvent<HTMLDivElement>) => {
    if (!dragged.current) return
    dragged.current = false
    event.preventDefault()
    event.stopPropagation()
  }
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return
    rememberFocus()
    move(event.key === 'ArrowLeft' ? -1 : 1)
    event.preventDefault()
  }
  if (!total) return null
  return (
    <div
      className={[styles.carousel, className].filter(Boolean).join(' ')}
      role="group"
      aria-roledescription="carousel"
      aria-labelledby={labelledBy}
      data-dragging={dragging}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={cancelDrag}
      onLostPointerCapture={() => {
        if (drag.current) cancelDrag()
      }}
      onDragStart={(event) => event.preventDefault()}
      onClickCapture={onClickCapture}
      onKeyDown={onKeyDown}
      onPointerEnter={(event) => {
        if (event.pointerType !== 'touch') setHovered(true)
      }}
      onPointerLeave={(event) => {
        setHovered(false)
        if (!event.currentTarget.hasPointerCapture?.(event.pointerId)) cancelDrag()
      }}
      onFocusCapture={() => setFocused(true)}
      onBlurCapture={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) setFocused(false)
      }}
    >
      <div
        className={styles.track}
        data-animated={animated && !reduced}
        style={
          { '--carousel-position': position, '--carousel-drag': `${dragOffset}px` } as CSSProperties
        }
        onTransitionEnd={(event) => {
          if (event.target === event.currentTarget) settle()
        }}
      >
        {[...items, ...items, ...items].map((item, index) => {
          const active = index === position
          const neighbour = total > 1 && Math.abs(index - position) === 1
          return (
            <div
              key={`${getKey(item)}-${index}`}
              ref={active ? activeSlide : undefined}
              tabIndex={active ? -1 : undefined}
              className={[
                styles.slide,
                active ? styles.active : index < position ? styles.peekStart : styles.peekEnd,
                slideClassName
              ]
                .filter(Boolean)
                .join(' ')}
              role={active ? 'group' : undefined}
              aria-roledescription={active ? 'slide' : undefined}
              aria-label={active ? labels.position(current + 1, total) : undefined}
              aria-live={
                active ? (autoplayDelay && !paused && !reduced ? 'off' : 'polite') : undefined
              }
              aria-hidden={!active || undefined}
              inert={!active || undefined}
            >
              {active || neighbour ? renderSlide(item, !active) : null}
            </div>
          )
        })}
      </div>
      {total > 1 ? (
        <>
          <div className={`${styles.arrow} ${styles.arrowStart}`}>
            <Button variant="quaternary" aria-label={labels.previous} onClick={() => move(-1)}>
              <GeneralLeftArrow size={24} aria-hidden="true" />
            </Button>
          </div>
          <div className={`${styles.arrow} ${styles.arrowEnd}`}>
            <Button variant="quaternary" aria-label={labels.next} onClick={() => move(1)}>
              <GeneralRightArrow size={24} aria-hidden="true" />
            </Button>
          </div>
          <div className={styles.dots}>
            {items.map((item, index) => (
              <button
                key={getKey(item)}
                type="button"
                className={index === current ? `${styles.dot} ${styles.dotActive}` : styles.dot}
                aria-label={labels.goTo(index + 1)}
                aria-current={index === current ? 'true' : undefined}
                onClick={() => go(index)}
              />
            ))}
          </div>
        </>
      ) : null}
    </div>
  )
}
export { Carousel }
export type { CarouselLabels, CarouselProps }
