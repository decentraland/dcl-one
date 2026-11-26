// AvatarDragSurface — the page's own hold on the engine's preview avatar: a transparent layer over it that
// turns the avatar while it is dragged and zooms its camera with the wheel, toward the height the pointer is
// at. The scene's drag-lock (the bridge scene's onMouseDragLocked) doesn't reach the preview in every host, so
// the page drives both and the scene only applies them (previewRotate / previewZoom). It sends at most once a
// frame, and on unmount gives back what it changed (the turn, and the zoom it last sent, even with a change
// still waiting for its frame), so the next page to show the preview starts at its own facing and framing.

import { useLayoutEffect, useRef } from 'react'
import { PREVIEW_MAX_ZOOM } from '@explorer-core/engine/protocol'
import styles from './AvatarDragSurface.module.css'

/** Degrees the avatar turns per CSS pixel dragged; negative turns it the way the pointer goes, as the
 *  scene's own drag did (the SDK7 AvatarPreviewElement's factor). */
export const DEGREES_PER_PX = -0.5
/** How fast the wheel zooms: the zoom scales by e^(−deltaY × this) per wheel event. */
const ZOOM_PER_DELTA = 0.0015

const nextFrame = (run: () => void): number =>
  typeof requestAnimationFrame === 'function' ? requestAnimationFrame(run) : (setTimeout(run, 16) as unknown as number)
const cancelFrame = (id: number): void => {
  if (typeof cancelAnimationFrame === 'function') cancelAnimationFrame(id)
  else clearTimeout(id)
}

export function AvatarDragSurface({
  onRotate,
  onZoom,
  view,
  className
}: {
  /** Turns the avatar by these degrees about its vertical axis. */
  onRotate: (degrees: number) => void
  /** Zooms the camera to this many times the framing in use, 1 to PREVIEW_MAX_ZOOM, keeping the height
   *  `anchor` of the camera's view (0 its top, 1 its bottom) where it is. */
  onZoom: (zoom: number, anchor: number) => void
  /** Where the camera's view is on screen, which the anchor is measured against: this layer's own box unless
   *  the page draws the view elsewhere (larger than the part it lets the pointer hold). */
  view?: () => DOMRect | null
  className?: string
}): React.JSX.Element {
  const handlers = useRef({ onRotate, onZoom, view })
  handlers.current = { onRotate, onZoom, view }
  const self = useRef<HTMLDivElement>(null)
  // The pointer dragging, and where it was when the last move was counted.
  const drag = useRef<{ pointer: number; x: number } | null>(null)
  // The zoom the wheel asks for, and the last one sent: unmounting resets what the scene has, the latter.
  const zoom = useRef(1)
  const sent = useRef(1)
  // The degrees sent so far, given back on unmount.
  const turned = useRef(0)
  const pending = useRef({ dx: 0, zoom: null as number | null, anchor: 0.5, frame: 0 })

  const flush = (): void => {
    const p = pending.current
    p.frame = 0
    if (p.dx !== 0) {
      const degrees = p.dx * DEGREES_PER_PX
      handlers.current.onRotate(degrees)
      turned.current += degrees
    }
    if (p.zoom != null) {
      handlers.current.onZoom(p.zoom, p.anchor)
      sent.current = p.zoom
    }
    p.dx = 0
    p.zoom = null
  }
  const schedule = (): void => {
    if (pending.current.frame === 0) pending.current.frame = nextFrame(flush)
  }

  // Given back in the layout cleanup: before the next page's EngineViewport reports its region (in its layout
  // effect), so the scene turns this page's avatar back, not one the next page already stood up (the lobby's).
  useLayoutEffect(
    () => () => {
      const p = pending.current
      if (p.frame !== 0) cancelFrame(p.frame)
      p.frame = 0
      if (turned.current !== 0) handlers.current.onRotate(-turned.current)
      if (sent.current !== 1) handlers.current.onZoom(1, 0.5)
    },
    []
  )

  const release = (pointer: number): void => {
    if (drag.current?.pointer === pointer) drag.current = null
  }

  return (
    <div
      ref={self}
      className={`${styles.surface} ${className ?? ''}`.trim()}
      aria-hidden="true"
      data-testid="avatar-drag-surface"
      onPointerDown={(e) => {
        if (e.button !== 0) return
        drag.current = { pointer: e.pointerId, x: e.clientX }
        // Kept while the pointer leaves the avatar mid-drag, as a turntable is.
        e.currentTarget.setPointerCapture?.(e.pointerId)
      }}
      onPointerMove={(e) => {
        const d = drag.current
        if (d == null || d.pointer !== e.pointerId) return
        pending.current.dx += e.clientX - d.x
        d.x = e.clientX
        schedule()
      }}
      onPointerUp={(e) => release(e.pointerId)}
      onPointerCancel={(e) => release(e.pointerId)}
      onLostPointerCapture={(e) => release(e.pointerId)}
      onWheel={(e) => {
        const next = Math.min(PREVIEW_MAX_ZOOM, Math.max(1, zoom.current * Math.exp(-e.deltaY * ZOOM_PER_DELTA)))
        if (next === zoom.current) return
        zoom.current = next
        const box = handlers.current.view?.() ?? self.current?.getBoundingClientRect() ?? null
        pending.current.anchor = box != null && box.height > 0 ? Math.min(1, Math.max(0, (e.clientY - box.top) / box.height)) : 0.5
        pending.current.zoom = next
        schedule()
      }}
    />
  )
}
