// The lobby's card rail: the strip slides a page (or a card) at a time by the arrows, the wheel and the
// dots, and freely by dragging, settling on the nearest card. The arrows and the dots sit in a pill
// under the cards, shown only while the rail is hovered and only when there is more than one page.
// The paging itself is railModel's.

import { Children, useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'
import arrowLeft from '../../assets/lobby/arrow-left.png'
import arrowRight from '../../assets/lobby/arrow-right.png'
import { MaskIcon } from '../../design'
import { DRAG_THRESHOLD, FLING_STALE_MS, RAIL_START, RailModel, WHEEL_COOLDOWN, type RailGeometry, type RailState } from './railModel'
import styles from './LobbyRail.module.css'

function measure(viewport: HTMLElement, strip: HTMLElement): RailGeometry {
  const cards = Array.from(strip.children) as HTMLElement[]
  return {
    lefts: cards.map((c) => c.offsetLeft),
    rights: cards.map((c) => c.offsetLeft + c.offsetWidth),
    viewport: viewport.clientWidth - parseFloat(getComputedStyle(viewport).paddingLeft || '0') - parseFloat(getComputedStyle(viewport).paddingRight || '0')
  }
}

export function LobbyRail({
  perPage,
  arrowsMoveOneCard = false,
  rewindKey,
  className,
  label,
  children
}: {
  perPage: number
  /** The arrows slide one card instead of a page; the wheel and the dots keep paging. */
  arrowsMoveOneCard?: boolean
  /** A change jumps back to the first page; otherwise new cards keep the page (clamped). */
  rewindKey?: unknown
  className?: string
  /** Names the rail for assistive technology. */
  label: string
  children: React.ReactNode
}): React.JSX.Element {
  const viewport = useRef<HTMLDivElement>(null)
  const strip = useRef<HTMLDivElement>(null)
  const count = Children.count(children)
  const [geometry, setGeometry] = useState<RailGeometry>({ lefts: [], rights: [], viewport: 0 })
  const [state, setState] = useState<RailState>(RAIL_START)
  const [instant, setInstant] = useState(false)
  const [dragOffset, setDragOffset] = useState<number | null>(null)
  const model = new RailModel(geometry, count, perPage, arrowsMoveOneCard)
  const modelRef = useRef(model)
  modelRef.current = model
  const lastWheel = useRef(-Infinity)

  useLayoutEffect(() => {
    const v = viewport.current
    const s = strip.current
    if (v == null || s == null) return
    const update = (): void => setGeometry(measure(v, s))
    update()
    if (typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver(update)
    observer.observe(v)
    observer.observe(s)
    return () => observer.disconnect()
  }, [count])

  // a new list rewinds without sliding; more or fewer cards keep the card in view
  useLayoutEffect(() => {
    setInstant(true)
    setState(RAIL_START)
  }, [rewindKey])
  useLayoutEffect(() => {
    setState((s) => modelRef.current.settle(s.card < count ? s : RAIL_START))
  }, [geometry, count])
  useEffect(() => {
    if (!instant) return
    const id = requestAnimationFrame(() => setInstant(false))
    return () => cancelAnimationFrame(id)
  }, [instant])

  const press = useRef<{ id: number; x: number; start: number; dragging: boolean; lastX: number; lastT: number; velocity: number } | null>(null)

  const onPointerDown = (e: React.PointerEvent): void => {
    if (e.button !== 0) return
    press.current = { id: e.pointerId, x: e.clientX, start: state.offset, dragging: false, lastX: e.clientX, lastT: e.timeStamp, velocity: 0 }
  }
  const scale = useCallback((): number => {
    const v = viewport.current
    return v == null ? 1 : v.getBoundingClientRect().width / (v.offsetWidth || 1) || 1
  }, [])
  const onPointerMove = (e: React.PointerEvent): void => {
    const p = press.current
    if (p == null || p.id !== e.pointerId) return
    if ((e.buttons & 1) === 0) {
      press.current = null
      return
    }
    const s = scale()
    if (!p.dragging) {
      if (Math.abs(e.clientX - p.x) / s < DRAG_THRESHOLD) return
      p.dragging = true
      p.x = e.clientX
      p.lastX = e.clientX
      p.lastT = e.timeStamp
      viewport.current?.setPointerCapture(e.pointerId)
      setInstant(true)
    }
    const dt = e.timeStamp - p.lastT
    if (dt > 0) {
      p.velocity = ((e.clientX - p.lastX) / s) * (1000 / dt)
      p.lastX = e.clientX
      p.lastT = e.timeStamp
    }
    setDragOffset(model.dragOffset(p.start, (e.clientX - p.x) / s))
  }
  const endPress = (e: React.PointerEvent): void => {
    const p = press.current
    if (p == null || p.id !== e.pointerId) return
    press.current = null
    if (!p.dragging) return
    const velocity = e.timeStamp - p.lastT > FLING_STALE_MS ? 0 : p.velocity
    setInstant(false)
    setState(model.dragEnd(dragOffset ?? p.start, velocity))
    setDragOffset(null)
  }
  // a drag never ends in a click on the card under it
  const onClickCapture = (e: React.MouseEvent): void => {
    if (dragJustEnded.current) {
      e.stopPropagation()
      e.preventDefault()
    }
  }
  const dragJustEnded = useRef(false)
  useEffect(() => {
    if (dragOffset != null) dragJustEnded.current = true
    else {
      const id = setTimeout(() => (dragJustEnded.current = false), 0)
      return () => clearTimeout(id)
    }
  }, [dragOffset])

  // a vertical wheel pages, at most once per snap; a horizontal-only scroll does nothing
  useEffect(() => {
    const v = viewport.current?.parentElement
    if (v == null) return
    const onWheel = (e: WheelEvent): void => {
      e.preventDefault()
      e.stopPropagation()
      if (e.deltaY === 0) return
      const now = performance.now() / 1000
      if (now - lastWheel.current < WHEEL_COOLDOWN) return
      lastWheel.current = now
      setState((s) => modelRef.current.wheel(s, e.deltaY > 0))
    }
    v.addEventListener('wheel', onWheel, { passive: false })
    return () => v.removeEventListener('wheel', onWheel)
  }, [])

  const pages = model.pageCount
  const offset = dragOffset ?? state.offset
  return (
    <div className={`${styles.rail} ${instant ? styles.instant : ''} ${className ?? ''}`.trim()} role="group" aria-roledescription="carousel" aria-label={label}>
      <div
        ref={viewport}
        className={styles.viewport}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={endPress}
        onPointerCancel={endPress}
        onLostPointerCapture={endPress}
        onClickCapture={onClickCapture}
      >
        <div ref={strip} className={styles.strip} style={{ transform: `translateX(${-offset}px)` }}>
          {children}
        </div>
      </div>
      <div className={styles.pager}>
        {pages > 1 && (
          <div className={styles.pill}>
            <button type="button" className={styles.arrow} aria-label="Previous" onClick={() => setState((s) => model.previous(s))}>
              <MaskIcon src={arrowLeft} width={9.4} height={16} />
            </button>
            <div className={styles.dots}>
              {Array.from({ length: pages }, (_, i) => (
                <button
                  key={i}
                  type="button"
                  className={`${styles.dot} ${i === state.page ? styles.dotSelected : ''}`.trim()}
                  aria-label={`Page ${i + 1}`}
                  aria-current={i === state.page ? 'true' : undefined}
                  onClick={() => setState(model.snapTo(i))}
                />
              ))}
            </div>
            <button type="button" className={styles.arrow} aria-label="Next" onClick={() => setState((s) => model.next(s))}>
              <MaskIcon src={arrowRight} width={9.4} height={16} />
            </button>
          </div>
        )}
      </div>
    </div>
  )
}
