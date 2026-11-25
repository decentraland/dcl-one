import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type RefObject } from 'react'
import { registerCancelLayer } from '@explorer-core/lib/cancelLayers'
import { hasOpenPopup } from '../../design'

/** A third of a navigation button: the 14px between its right edge and what opens beside it. */
const GAP = 1 / 3

/** What a navigation button opens beside the bar, kept on screen and closed by an outside press
 *  or Cancel. Put `ref` and `style` on the panel, which is `position: fixed`.
 *
 *  Without `rise` (scene permissions, portable experiences, nearby voice) the panel is centred on
 *  the button: its CSS adds translateY(-50%) around its top (PermissionsPopover.module.css
 *  `.panel`). With `rise` (the Me, Discover, Shop and System panels) the panel's top lies `rise`
 *  reference px above the button's top, and its CSS adds no translate.
 *
 *  `closer` selects a control outside the panel whose own press closes it (the Map button): a
 *  press there is not an outside press, or that control would read a state its press had changed. */
export function useRailPopover(anchor: HTMLElement | null, onClose: () => void, rise?: number, closer?: string): { ref: RefObject<HTMLDivElement | null>; style: CSSProperties } {
  const ref = useRef<HTMLDivElement>(null)
  const [top, setTop] = useState<number | null>(null)
  const [left, setLeft] = useState(0)

  // After every render, since the content sets the height: clamp by the measured (already
  // scaled) height so a long list stays on screen.
  useLayoutEffect(() => {
    if (anchor == null || ref.current == null) return
    const r = anchor.getBoundingClientRect()
    const height = ref.current.getBoundingClientRect().height
    if (rise == null) {
      const half = height / 2
      setTop(Math.max(half + 8, Math.min(r.top + r.height / 2, window.innerHeight - half - 8)))
    } else {
      // the button is 42 reference px high, so its measured height carries the HUD's scale
      const wanted = r.top - rise * (r.height / 42)
      setTop(Math.max(8, Math.min(wanted, window.innerHeight - height - 8)))
    }
    setLeft(r.right + r.width * GAP)
  })

  // pointerdown so a click on the world closes it too (the canvas suppresses mousedown)
  useEffect(() => {
    const onDown = (e: PointerEvent): void => {
      const t = e.target as Node
      if (ref.current?.contains(t) || anchor?.contains(t)) return
      if (closer != null && t instanceof Element && t.closest(closer) != null) return
      // a dialog over the panel (the Me panel's sign-out question) takes the press: it is not one outside
      if (hasOpenPopup()) return
      onClose()
    }
    document.addEventListener('pointerdown', onDown, true)
    const off = registerCancelLayer(onClose)
    return () => {
      document.removeEventListener('pointerdown', onDown, true)
      off()
    }
  }, [anchor, onClose, closer])

  return { ref, style: { top: top ?? -9999, left } }
}
