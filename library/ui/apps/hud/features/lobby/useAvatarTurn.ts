// useAvatarTurn — the lobby avatar as a turntable: the pointer handlers for the element over it. A drag
// across it turns the avatar; a press that stays within DRAG_SLOP is still a click on that element.

import { useRef } from 'react'
import { DEGREES_PER_PX } from '../engine/AvatarDragSurface'

/** CSS pixels a press may move and still be a click. */
const DRAG_SLOP = 4

type Press = React.PointerEvent<HTMLElement>

export function useAvatarTurn(turn: (degrees: number) => void): {
  /** The press that ended in this click turned the avatar: not a click on it. A click from the
   *  keyboard (`detail` 0) has no press, and each drag cancels one click only. */
  wasDrag: (click: { detail: number }) => boolean
  handlers: Record<'onPointerDown' | 'onPointerMove' | 'onPointerUp' | 'onPointerCancel' | 'onLostPointerCapture', (e: Press) => void>
} {
  const press = useRef<{ pointer: number; x: number } | null>(null)
  const dragged = useRef(false)
  const release = (e: Press): void => {
    if (press.current?.pointer === e.pointerId) press.current = null
  }
  return {
    wasDrag: (click) => {
      const was = dragged.current
      dragged.current = false
      return was && click.detail !== 0
    },
    handlers: {
      onPointerDown: (e) => {
        if (e.button !== 0) return
        press.current = { pointer: e.pointerId, x: e.clientX }
        dragged.current = false
        // Kept while the pointer leaves the avatar mid-drag, as a turntable is.
        e.currentTarget.setPointerCapture?.(e.pointerId)
      },
      onPointerMove: (e) => {
        const p = press.current
        if (p == null || p.pointer !== e.pointerId) return
        const dx = e.clientX - p.x
        if (!dragged.current && Math.abs(dx) < DRAG_SLOP) return
        dragged.current = true
        p.x = e.clientX
        turn(dx * DEGREES_PER_PX)
      },
      onPointerUp: release,
      onPointerCancel: release,
      onLostPointerCapture: release
    }
  }
}
