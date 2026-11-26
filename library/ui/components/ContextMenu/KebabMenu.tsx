import { useCallback, useEffect, useRef, useState } from 'react'
import { Button } from '../Button/Button'
import { ContextMenu, type ContextMenuItem, type MenuWhileOpen } from './ContextMenu'
import styles from './ContextMenu.module.css'

export interface KebabMenuProps {
  /** The button's accessible name. */
  label: string
  /** `close` shuts the menu; an entry calls it before its action. */
  items: (close: () => void) => ContextMenuItem[]
  size?: 'sm' | 'md'
  /** However the menu closed. */
  onClosed?: () => void
  whileOpen?: MenuWhileOpen
}

/** A kebab button that opens a ContextMenu below it, right-aligned. It closes on a press outside. */
export function KebabMenu({ label, items, size = 'md', onClosed, whileOpen }: KebabMenuProps) {
  const [open, setOpen] = useState(false)
  const root = useRef<HTMLDivElement>(null)
  const closed = useRef(onClosed)
  const opened = useRef(whileOpen)
  useEffect(() => {
    closed.current = onClosed
    opened.current = whileOpen
  })
  const close = useCallback(() => {
    setOpen(false)
    closed.current?.()
  }, [])
  useEffect(() => {
    if (!open) return
    const press = (event: PointerEvent): void => {
      if (root.current != null && !root.current.contains(event.target as Node)) close()
    }
    document.addEventListener('pointerdown', press, true)
    const off = opened.current?.(close)
    return () => {
      document.removeEventListener('pointerdown', press, true)
      off?.()
    }
  }, [open, close])
  const dots = size === 'sm' ? 16 : 18
  return (
    <div ref={root} className={styles.kebabWrap}>
      <Button
        appearance="hud"
        control="square"
        variant="faint"
        size={size}
        aria-label={label}
        aria-expanded={open}
        active={open}
        onClick={() => (open ? close() : setOpen(true))}
      >
        <svg viewBox="0 0 24 24" width={dots} height={dots} fill="currentColor" aria-hidden="true">
          <circle cx="12" cy="5" r="2" />
          <circle cx="12" cy="12" r="2" />
          <circle cx="12" cy="19" r="2" />
        </svg>
      </Button>
      {open && (
        <div className={styles.kebabMenu}>
          <ContextMenu items={items(close)} whileSubmenuOpen={whileOpen} />
        </div>
      )}
    </div>
  )
}
