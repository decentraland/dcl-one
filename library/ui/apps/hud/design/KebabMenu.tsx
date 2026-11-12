// KebabMenu — a kebab (⋮) button that opens a ContextMenu below it, right-aligned. Closes on an
// outside click, Cancel, or when an entry asks it to (`close`, handed to `items`).
import { useCallback, useEffect, useRef, useState } from 'react'
import { registerCancelLayer } from '@explorer-core/lib/cancelLayers'
import { ContextMenu, type ContextMenuItem } from './ContextMenu'
import { ControlButton } from './ControlButton'
import { Kebab } from './Glyphs'
import styles from './KebabMenu.module.css'

export function KebabMenu({
  label,
  items,
  size = 'md',
  onClosed
}: {
  label: string
  /** `close` shuts the menu; an entry calls it before its action. */
  items: (close: () => void) => ContextMenuItem[]
  size?: 'sm' | 'md'
  /** However the menu closed (e.g. to reset a submenu page). */
  onClosed?: () => void
}): React.JSX.Element {
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  const onClosedRef = useRef(onClosed)
  onClosedRef.current = onClosed
  const close = useCallback(() => {
    setOpen(false)
    onClosedRef.current?.()
  }, [])
  useEffect(() => {
    if (!open) return
    const onDown = (e: PointerEvent): void => {
      if (ref.current != null && !ref.current.contains(e.target as Node)) close()
    }
    document.addEventListener('pointerdown', onDown, true)
    const off = registerCancelLayer(close)
    return () => {
      document.removeEventListener('pointerdown', onDown, true)
      off()
    }
  }, [open, close])
  return (
    <div ref={ref} className={styles.wrap}>
      <ControlButton
        variant="faint"
        size={size}
        aria-label={label}
        aria-expanded={open}
        active={open}
        onClick={() => (open ? close() : setOpen(true))}
      >
        <Kebab vertical size={size === 'sm' ? 16 : 18} r={2} />
      </ControlButton>
      {open && (
        <div className={styles.menu}>
          <ContextMenu items={items(close)} />
        </div>
      )}
    </div>
  )
}
