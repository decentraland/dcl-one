// Submenu — a menu row that opens a list of further items beside it: on hover, on click, or from
// the keyboard (ArrowRight, Enter or Space opens it and focuses its first item; ArrowDown and
// ArrowUp move through the items; ArrowLeft or Escape closes it and returns to the row). The list is
// the children, usually a scrolling ContextMenu. The row takes the host menu's own row style
// through `className`.

import { useId, useLayoutEffect, useRef, useState, type ReactNode } from 'react'
import styles from './Submenu.module.css'

export interface SubmenuProps {
  label: ReactNode
  icon?: ReactNode
  /** The list it opens, e.g. `<ContextMenu items={…} scroll />`. */
  children: ReactNode
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

const menuItems = (list: HTMLElement | null): HTMLElement[] =>
  list == null ? [] : Array.from(list.querySelectorAll<HTMLElement>('[role^="menuitem"]:not([aria-disabled="true"]):not(:disabled)'))

export function Submenu({ label, icon, children, className }: SubmenuProps): React.JSX.Element {
  const [open, setOpen] = useState(false)
  // opens to the left when the right has no room for it
  const [left, setLeft] = useState(false)
  // the keyboard opened it: its first item takes focus
  const [focusFirst, setFocusFirst] = useState(false)
  const flyout = useRef<HTMLDivElement>(null)
  const row = useRef<HTMLButtonElement>(null)
  const listId = useId()
  useLayoutEffect(() => {
    if (!open) {
      setLeft(false)
      return
    }
    const el = flyout.current
    if (el == null) return
    if (el.getBoundingClientRect().right > clipRight(el)) setLeft(true)
    if (focusFirst) {
      menuItems(el)[0]?.focus()
      setFocusFirst(false)
    }
  }, [open, focusFirst])
  const close = (): void => {
    setOpen(false)
    row.current?.focus()
  }
  const onRowKey = (e: React.KeyboardEvent): void => {
    if (e.key === 'ArrowRight' || e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      setFocusFirst(true)
      setOpen(true)
    }
  }
  const onListKey = (e: React.KeyboardEvent): void => {
    if (e.key === 'Escape' || e.key === 'ArrowLeft') {
      // the submenu closes, not the menu it sits in
      e.preventDefault()
      e.stopPropagation()
      close()
      return
    }
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return
    e.preventDefault()
    const list = menuItems(flyout.current)
    if (list.length === 0) return
    const at = list.indexOf(document.activeElement as HTMLElement)
    const next = e.key === 'ArrowDown' ? (at + 1) % list.length : (at - 1 + list.length) % list.length
    list[next]?.focus()
  }
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
        ref={row}
        type="button"
        className={`${styles.row} ${className ?? ''}`.trim()}
        role="menuitem"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        // a click opens it (hovering already may have); leaving the row or its list closes it
        onClick={() => setOpen(true)}
        onKeyDown={onRowKey}
      >
        {icon != null && <span className={styles.icon}>{icon}</span>}
        <span className={styles.label}>{label}</span>
        <span className={styles.chev} aria-hidden="true">›</span>
      </button>
      {open && (
        <div id={listId} ref={flyout} className={`${styles.flyout} ${left ? styles.flyoutLeft : ''}`.trim()} onKeyDown={onListKey}>
          {children}
        </div>
      )}
    </div>
  )
}
