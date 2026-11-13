// The map's camera. Its position and zoom live outside React state: every change is written
// straight onto the viewport element as custom properties, which the atlases and markers
// position themselves from, so a drag or a zoom step repaints without re-rendering the page.

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { PARCEL_UNITS, coordsToPosition, parcelWithinBounds, type Vec2 } from '@explorer-core/features/map/mapCoords'
import {
  BASE_ZOOM,
  DEFAULT_ZOOM_LEVEL,
  DRAG_DECELERATION_RATE,
  MAX_ZOOM_LEVEL,
  ZOOM_STEP_SECONDS,
  clampCamera,
  clampZoomLevel,
  easeOutQuart,
  normalizedZoom,
  orthoFor,
  pathDotSize,
  pinMarkerScale,
  pxPerUnit,
  screenToMap,
  titleForScale,
  userMarkerScale,
  type MapView
} from '@explorer-core/features/map/mapZoom'

/** How far under its marker a title sits, in marker units. */
const TITLE_BASE_Y = -1.2

export interface MapCamera {
  viewportRef: (el: HTMLDivElement | null) => void
  /** Where the camera is right now, for what draws itself each frame. */
  view: { current: MapView }
  level: number
  /** The map-unit rectangle in view, refreshed when it moves by a meaningful amount. */
  visible: { xMin: number; yMin: number; xMax: number; yMax: number }
  setPosition: (coords: Vec2) => void
  translateTo: (coords: Vec2, seconds: number) => void
  /** One zoom step; ignored while a step is running or zoom is blocked. */
  zoom: (zoomIn: boolean) => void
  resetZoom: () => void
  blockZoom: (blocked: boolean) => void
  /** The parcel under a pointer event, or null outside the city or the viewport. */
  parcelAt: (clientX: number, clientY: number) => Vec2 | null
  beginDrag: (clientX: number, clientY: number) => void
  drag: (clientX: number, clientY: number) => void
  endDrag: () => void
}

interface DragState {
  startX: number
  startY: number
  camX: number
  camY: number
  ratio: number
}

export function useMapCamera(): MapCamera {
  const elRef = useRef<HTMLDivElement | null>(null)
  const view = useRef<MapView>({ x: 0, y: 0, ortho: orthoFor(normalizedZoom(DEFAULT_ZOOM_LEVEL)), width: 1920, height: 1010 })
  const zoomValue = useRef(normalizedZoom(DEFAULT_ZOOM_LEVEL))
  const [level, setLevel] = useState(DEFAULT_ZOOM_LEVEL)
  const levelRef = useRef(DEFAULT_ZOOM_LEVEL)
  const [visible, setVisible] = useState({ xMin: -1e9, yMin: -1e9, xMax: 1e9, yMax: 1e9 })
  const visibleRef = useRef(visible)
  const scaling = useRef(false)
  const zoomBlocked = useRef(false)
  const dragState = useRef<DragState | null>(null)
  const glide = useRef(0)
  const inertia = useRef(0)
  const velocity = useRef({ x: 0, y: 0 })

  const write = useCallback((): void => {
    const el = elRef.current
    const v = view.current
    if (el == null) return
    const k = pxPerUnit(v)
    const ratio = v.ortho / BASE_ZOOM
    const title = titleForScale(TITLE_BASE_Y, 1, PARCEL_UNITS, ratio * PARCEL_UNITS)
    const s = el.style
    s.setProperty('--cam-x', String(v.x))
    s.setProperty('--cam-y', String(v.y))
    s.setProperty('--k', String(k))
    s.setProperty('--unit', `${ratio * PARCEL_UNITS * k}px`)
    s.setProperty('--parcel', `${PARCEL_UNITS * k}px`)
    s.setProperty('--title-scale', String(title.scale))
    s.setProperty('--title-y', String(title.y))
    s.setProperty('--user-unit', `${userMarkerScale(v.ortho) * k}px`)
    s.setProperty('--pin-unit', `${pinMarkerScale(v.ortho) * k}px`)
    s.setProperty('--path-dot', `${pathDotSize(v.ortho) * k}px`)

    // tell React only once the view has moved a quarter of its height past the rectangle it last
    // heard about, which carries half a height of margin on every side
    const halfH = v.ortho
    const halfW = v.ortho * (v.width / v.height)
    const seen = visibleRef.current
    if (v.x - halfW < seen.xMin + halfH * 0.25 || v.x + halfW > seen.xMax - halfH * 0.25 || v.y - halfH < seen.yMin + halfH * 0.25 || v.y + halfH > seen.yMax - halfH * 0.25 || seen.yMax - seen.yMin > halfH * 6) {
      const next = { xMin: v.x - halfW - halfH, yMin: v.y - halfH * 2, xMax: v.x + halfW + halfH, yMax: v.y + halfH * 2 }
      visibleRef.current = next
      setVisible(next)
    }
  }, [])

  const place = useCallback(
    (x: number, y: number): void => {
      const v = view.current
      const clamped = clampCamera({ x, y }, v.ortho, v.width / v.height)
      v.x = clamped.x
      v.y = clamped.y
      write()
    },
    [write]
  )

  const setZoomValue = useCallback(
    (normalized: number): void => {
      zoomValue.current = normalized
      view.current.ortho = orthoFor(normalized)
      place(view.current.x, view.current.y)
    },
    [place]
  )

  const stopMotion = useCallback((): void => {
    cancelAnimationFrame(glide.current)
    cancelAnimationFrame(inertia.current)
  }, [])

  const viewportRef = useCallback(
    (el: HTMLDivElement | null): void => {
      elRef.current = el
      if (el == null) return
      view.current.width = el.offsetWidth || view.current.width
      view.current.height = el.offsetHeight || view.current.height
      write()
    },
    [write]
  )

  useEffect(() => {
    const el = elRef.current
    if (el == null || typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver(() => {
      if (el.offsetWidth === 0 || el.offsetHeight === 0) return
      view.current.width = el.offsetWidth
      view.current.height = el.offsetHeight
      place(view.current.x, view.current.y)
    })
    observer.observe(el)
    return () => observer.disconnect()
  }, [place])

  useEffect(() => stopMotion, [stopMotion])

  const setPosition = useCallback(
    (coords: Vec2): void => {
      stopMotion()
      const pos = coordsToPosition(coords)
      place(pos.x, pos.y)
    },
    [place, stopMotion]
  )

  const translateTo = useCallback(
    (coords: Vec2, seconds: number): void => {
      stopMotion()
      const v = view.current
      const target = clampCamera(coordsToPosition(coords), v.ortho, v.width / v.height)
      const from = { x: v.x, y: v.y }
      const start = performance.now()
      const step = (now: number): void => {
        const t = Math.min(1, (now - start) / (seconds * 1000))
        const e = easeOutQuart(t)
        // the glide is not clamped on the way: it ends on a clamped target
        v.x = from.x + (target.x - from.x) * e
        v.y = from.y + (target.y - from.y) * e
        write()
        if (t < 1) glide.current = requestAnimationFrame(step)
      }
      glide.current = requestAnimationFrame(step)
    },
    [stopMotion, write]
  )

  const zoom = useCallback(
    (zoomIn: boolean): void => {
      if (scaling.current || zoomBlocked.current) return
      const current = levelRef.current
      if ((zoomIn && current >= MAX_ZOOM_LEVEL) || (!zoomIn && current <= 0)) return
      const next = clampZoomLevel(current + (zoomIn ? 1 : -1))
      levelRef.current = next
      setLevel(next)
      const from = zoomValue.current
      const to = normalizedZoom(next)
      const start = performance.now()
      scaling.current = true
      const step = (now: number): void => {
        const t = Math.min(1, (now - start) / (ZOOM_STEP_SECONDS * 1000))
        setZoomValue(from + (to - from) * t)
        if (t < 1) requestAnimationFrame(step)
        else scaling.current = false
      }
      requestAnimationFrame(step)
    },
    [setZoomValue]
  )

  const resetZoom = useCallback((): void => {
    levelRef.current = DEFAULT_ZOOM_LEVEL
    setLevel(DEFAULT_ZOOM_LEVEL)
    setZoomValue(normalizedZoom(DEFAULT_ZOOM_LEVEL))
  }, [setZoomValue])

  const blockZoom = useCallback((blocked: boolean): void => {
    zoomBlocked.current = blocked
  }, [])

  // The page is drawn inside a scaled overlay, so a client px is not a layout px.
  const layoutScale = (el: HTMLDivElement): number => (el.offsetWidth > 0 ? el.getBoundingClientRect().width / el.offsetWidth : 1)

  const parcelAt = useCallback((clientX: number, clientY: number): Vec2 | null => {
    const el = elRef.current
    if (el == null) return null
    const rect = el.getBoundingClientRect()
    const scale = layoutScale(el)
    const point = { x: (clientX - rect.left) / scale, y: (clientY - rect.top) / scale }
    const v = view.current
    if (point.x < 0 || point.y < 0 || point.x > v.width || point.y > v.height) return null
    return parcelWithinBounds(screenToMap(point, v))
  }, [])

  const beginDrag = useCallback(
    (clientX: number, clientY: number): void => {
      const el = elRef.current
      if (el == null) return
      stopMotion()
      const v = view.current
      // map units per client px
      const ratio = (v.ortho * 2) / (v.height * layoutScale(el))
      dragState.current = { startX: clientX, startY: clientY, camX: v.x, camY: v.y, ratio }
      velocity.current = { x: 0, y: 0 }
      let last = { x: v.x, y: v.y }
      let lastTime = performance.now()
      const loop = (now: number): void => {
        const dt = Math.max(0.001, (now - lastTime) / 1000)
        lastTime = now
        const cam = view.current
        if (dragState.current != null) {
          const blend = Math.min(1, dt * 10)
          velocity.current = {
            x: velocity.current.x + ((cam.x - last.x) / dt - velocity.current.x) * blend,
            y: velocity.current.y + ((cam.y - last.y) / dt - velocity.current.y) * blend
          }
        } else {
          const keep = Math.pow(DRAG_DECELERATION_RATE, dt)
          let vx = velocity.current.x * keep
          let vy = velocity.current.y * keep
          if (Math.abs(vx) < 1) vx = 0
          if (Math.abs(vy) < 1) vy = 0
          velocity.current = { x: vx, y: vy }
          place(cam.x + vx * dt, cam.y + vy * dt)
          if (vx === 0 && vy === 0) return
        }
        last = { x: cam.x, y: cam.y }
        inertia.current = requestAnimationFrame(loop)
      }
      inertia.current = requestAnimationFrame(loop)
    },
    [place, stopMotion]
  )

  const drag = useCallback(
    (clientX: number, clientY: number): void => {
      const d = dragState.current
      if (d == null) return
      place(d.camX - (clientX - d.startX) * d.ratio, d.camY + (clientY - d.startY) * d.ratio)
    },
    [place]
  )

  const endDrag = useCallback((): void => {
    dragState.current = null
  }, [])

  return useMemo(
    () => ({ viewportRef, view, level, visible, setPosition, translateTo, zoom, resetZoom, blockZoom, parcelAt, beginDrag, drag, endDrag }),
    [viewportRef, level, visible, setPosition, translateTo, zoom, resetZoom, blockZoom, parcelAt, beginDrag, drag, endDrag]
  )
}
