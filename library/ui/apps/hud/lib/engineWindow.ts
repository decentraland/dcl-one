// The engine's window through the popup layer: where a popup shows something the engine draws under the
// page (the passport's avatar), every layer of the page under that spot has to be see-through. The popup
// that owns the window says where it is; the popup layer cuts it out of the popups under it and out of the
// owner's own scrim, and App cuts it out of everything under the popups.

import { useSyncExternalStore } from 'react'
import { holeClip } from '@explorer-core/lib/holeClip'

type Rect = { x: number; y: number; width: number; height: number }
type Corners = { topLeft?: number; topRight?: number; bottomRight?: number; bottomLeft?: number }

export type EngineWindow = {
  /** The popup that owns it (its PopupIdContext). */
  popup: number
  /** On screen, in CSS pixels. */
  rect: Rect
  round: Corners
}

let current: EngineWindow | null = null
const listeners = new Set<() => void>()

const same = (a: EngineWindow | null, b: EngineWindow | null): boolean =>
  a === b ||
  (a != null &&
    b != null &&
    a.popup === b.popup &&
    a.rect.x === b.rect.x &&
    a.rect.y === b.rect.y &&
    a.rect.width === b.rect.width &&
    a.rect.height === b.rect.height &&
    a.round.topLeft === b.round.topLeft &&
    a.round.topRight === b.round.topRight &&
    a.round.bottomRight === b.round.bottomRight &&
    a.round.bottomLeft === b.round.bottomLeft)

export function setEngineWindow(next: EngineWindow | null): void {
  if (same(current, next)) return
  current = next
  listeners.forEach((listener) => listener())
}

const subscribe = (listener: () => void): (() => void) => {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export const useEngineWindow = (): EngineWindow | null => useSyncExternalStore(subscribe, () => current, () => null)

/** The window as a clip-path for a layer whose box starts at `origin` on screen and is drawn at `scale`. */
export function windowClip(window: EngineWindow, origin: { x: number; y: number } = { x: 0, y: 0 }, scale = 1): string {
  const s = scale > 0 ? scale : 1
  const at = (v: number | undefined): number => (v ?? 0) / s
  const { rect, round } = window
  const hole = holeClip(
    { x: (rect.x - origin.x) / s, y: (rect.y - origin.y) / s, width: rect.width / s, height: rect.height / s },
    { topLeft: at(round.topLeft), topRight: at(round.topRight), bottomRight: at(round.bottomRight), bottomLeft: at(round.bottomLeft) }
  ).clipPath as string
  // holeClip's outer box is the layer's own; these layers have no box of their own (a wrapper of fixed
  // children), so the outer box is spelled out, far past any screen.
  return hole.replace('0 0, 100% 0, 100% 100%, 0 100%, 0 0,', `${-FAR}px ${-FAR}px, ${FAR}px ${-FAR}px, ${FAR}px ${FAR}px, ${-FAR}px ${FAR}px, ${-FAR}px ${-FAR}px,`).replace(/, 0 0\)$/, `, ${-FAR}px ${-FAR}px)`)
}
const FAR = 100000
