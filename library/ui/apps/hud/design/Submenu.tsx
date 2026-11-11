// Submenu — a menu row that opens a list of further items beside it, on hover or click. The
// list is a ContextMenu, scrolled past its maximum height. The row takes the host menu's own row style through `className`.

import { useId, useLayoutEffect, useRef, useState, type ReactNode } from 'react'
import { ContextMenu, type ContextMenuItem } from './ContextMenu'
import styles from './Submenu.module.css'

export interface SubmenuProps {
  label: ReactNode
  icon?: ReactNode
  items: ContextMenuItem[]
  /** The host menu's row style, so the row matches its siblings. */
  className?: string
}

/** Where the list stops being visible on the right: the window's edge, or that of the nearest
 *  ancestor that clips its overflow (a passport, a scrolling panel). */
function clipRight(el: HTMLElement): number {
  let right = window.innerWidth
  for (let p = el.parentElement; p != null; p = p.parentElement) {
    if (getComputedStyle(p).overflowX !== 'visible') {
      right = Math.min(right, p.getBoundingClientRect().right)
      break
    }
  }
  return right
}

export function Submenu({ label, icon, items, className }: SubmenuProps): React.JSX.Element {
  const [open, setOpen] = useState(false)
  // opens to the left when the right has no room for it
  const [left, setLeft] = useState(false)
  const flyout = useRef<HTMLDivElement>(null)
  const listId = useId()
  useLayoutEffect(() => {
    if (!open) {
      setLeft(false)
      return
    }
    const el = flyout.current
    if (el == null) return
    if (el.getBoundingClientRect().right > clipRight(el)) setLeft(true)
  }, [open])
  return (
    <div
      className={styles.wrap}
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setOpen(false)
      }}
    >
      <button
        type="button"
        className={`${styles.row} ${className ?? ''}`.trim()}
        role="menuitem"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        // a click opens it (hovering already may have); leaving the row or its list closes it
        onClick={() => setOpen(true)}
      >
        {icon != null && <span className={styles.icon}>{icon}</span>}
        <span className={styles.label}>{label}</span>
        <span className={styles.chev} aria-hidden="true">›</span>
      </button>
      {open && (
        <div id={listId} ref={flyout} className={`${styles.flyout} ${left ? styles.flyoutLeft : ''}`.trim()}>
          <ContextMenu items={items} scroll />
        </div>
      )}
    </div>
  )
}
