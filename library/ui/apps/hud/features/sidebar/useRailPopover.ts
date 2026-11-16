import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type RefObject } from 'react'
import { registerCancelLayer } from '@explorer-core/lib/cancelLayers'

/** A rail popover (scene permissions, portable experiences): centred on the button that opened it and kept
 *  on screen, closed by an outside click or Cancel. Put `ref` and `style` on the panel, which is
 *  `position: fixed` and translateY(-50%) around its top (PermissionsPopover.module.css `.panel`). */
export function useRailPopover(anchor: HTMLElement | null, onClose: () => void): { ref: RefObject<HTMLDivElement | null>; style: CSSProperties } {
  const ref = useRef<HTMLDivElement>(null)
  const [top, setTop] = useState<number | null>(null)
  const [left, setLeft] = useState(0)

  // After every render, since the content sets the height: clamp the centre by the measured (already
  // scaled) height so a long list stays on screen.
  useLayoutEffect(() => {
    if (anchor == null || ref.current == null) return
    const r = anchor.getBoundingClientRect()
    const half = ref.current.getBoundingClientRect().height / 2
    setTop(Math.max(half + 8, Math.min(r.top + r.height / 2, window.innerHeight - half - 8)))
    setLeft(r.left + r.width / 2 + r.width)
  })

  // pointerdown so a click on the world closes it too (the canvas suppresses mousedown)
  useEffect(() => {
    const onDown = (e: PointerEvent): void => {
      const t = e.target as Node
      if (ref.current?.contains(t) || anchor?.contains(t)) return
      onClose()
    }
    document.addEventListener('pointerdown', onDown, true)
    const off = registerCancelLayer(onClose)
    return () => {
      document.removeEventListener('pointerdown', onDown, true)
      off()
    }
  }, [anchor, onClose])

  return { ref, style: { top: top ?? -9999, left } }
}
