// Submenu — a menu row that opens a list of further items beside it, on hover or click. A click
// from the keyboard (Enter or Space on the focused row) also moves focus to the list's first item,
// and Tab moves through the list. Cancel closes it through the cancel-layer stack, so one press
// closes the list, not the menu it sits in. The list is the children, usually a scrolling
// ContextMenu. It opens to the right and down, flipping left or up where that side has no room.
// The row takes the host menu's own row style through `className`.

import { useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode } from 'react'
import { registerCancelLayer } from '@explorer-core/lib/cancelLayers'
import styles from './Submenu.module.css'

export interface SubmenuProps {
  label: ReactNode
  icon?: ReactNode
  /** The list it opens, e.g. `<ContextMenu items={…} scroll />`. */
  children: ReactNode
  /** The host menu's row style, so the row matches its siblings. */
  className?: string
}

/** Where the list stops being visible: the window's edges, or those of the nearest ancestor that
 *  clips its overflow (a passport, a scrolling panel). */
function clipEdges(el: HTMLElement): { right: number; bottom: number } {
  let right = window.innerWidth
  let bottom = window.innerHeight
  for (let p = el.parentElement; p != null; p = p.parentElement) {
    const style = getComputedStyle(p)
    if (style.overflowX !== 'visible' || style.overflowY !== 'visible') {
      const r = p.getBoundingClientRect()
      right = Math.min(right, r.right)
      bottom = Math.min(bottom, r.bottom)
      break
    }
  }
  return { right, bottom }
}

export function Submenu({ label, icon, children, className }: SubmenuProps): React.JSX.Element {
  const [open, setOpen] = useState(false)
  // where the list has no room: it opens to the left, or upward
  const [flip, setFlip] = useState({ left: false, up: false })
  // a keyboard click opened it: its first item takes focus
  const [focusFirst, setFocusFirst] = useState(false)
  const flyout = useRef<HTMLDivElement>(null)
  const row = useRef<HTMLButtonElement>(null)
  const listId = useId()
  useLayoutEffect(() => {
    if (!open) {
      setFlip({ left: false, up: false })
      return
    }
    const el = flyout.current
    if (el == null) return
    const r = el.getBoundingClientRect()
    const edges = clipEdges(el)
    setFlip({ left: r.right > edges.right, up: r.bottom > edges.bottom })
    if (focusFirst) {
      el.querySelector<HTMLElement>('[role^="menuitem"]:not([aria-disabled="true"]):not(:disabled)')?.focus()
      setFocusFirst(false)
    }
  }, [open, focusFirst])
  // Cancel (Escape, the gamepad's back) closes the list first, then the menu on the next press
  useEffect(() => {
    if (!open) return
    return registerCancelLayer(() => {
      setOpen(false)
      row.current?.focus()
    })
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
        ref={row}
        type="button"
        className={`${styles.row} ${className ?? ''}`.trim()}
        role="menuitem"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        // a click opens it (hovering already may have); one from the keyboard (detail 0) also
        // focuses the list. Leaving the row or its list closes it.
        onClick={(e) => {
          if (e.detail === 0) setFocusFirst(true)
          setOpen(true)
        }}
      >
        {icon != null && <span className={styles.icon}>{icon}</span>}
        <span className={styles.label}>{label}</span>
        <span className={styles.chev} aria-hidden="true">›</span>
      </button>
      {open && (
        <div
          id={listId}
          ref={flyout}
          className={`${styles.flyout} ${flip.left ? styles.flyoutLeft : ''} ${flip.up ? styles.flyoutUp : ''}`.replace(/\s+/g, ' ').trim()}
        >
          {children}
        </div>
      )}
    </div>
  )
}
