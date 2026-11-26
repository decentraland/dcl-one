import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { ContextMenu, type ContextMenuItem, type MenuWhileOpen } from './ContextMenu'
import styles from './ContextMenu.module.css'

const OFFSET_X = 11
const OFFSET_Y = 18

/** Where a menu of `size` opens from `anchor` inside `bounds`, all in the same unscaled px. */
export function anchoredMenuPosition(
  anchor: { x: number; y: number },
  size: { width: number; height: number },
  bounds: { width: number; height: number }
): { left: number; top: number } {
  let left = anchor.x + OFFSET_X
  let top = anchor.y - OFFSET_Y
  if (left + size.width > bounds.width) left = anchor.x - OFFSET_X - size.width
  if (top + size.height > bounds.height) top = anchor.y + OFFSET_Y - size.height
  left = Math.min(Math.max(0, left), Math.max(0, bounds.width - size.width))
  top = Math.min(Math.max(0, top), Math.max(0, bounds.height - size.height))
  return { left, top }
}

function closesMenu(item: ContextMenuItem): item is Extract<ContextMenuItem, { onClick?: () => void }> {
  return item.kind == null || item.kind === 'item' || item.kind === 'submenu' || item.kind === 'option'
}

function focusables(node: HTMLElement | null): HTMLElement[] {
  // A switch row's control is a native checkbox.
  return Array.from(
    node?.querySelectorAll<HTMLElement>('button:not(:disabled), input[role="switch"]:not(:disabled)') ?? []
  )
}

export interface AnchoredMenuProps {
  /** Closed, it draws nothing and listens to nothing. Open by default: most callers mount it only while open. */
  open?: boolean
  /** The point the menu opens from, in viewport px. */
  anchor: { x: number; y: number }
  items: ContextMenuItem[]
  onClose: () => void
  /** An element whose presses do not count as outside (the button that toggles the menu). */
  ignore?: Element | null
  /** The scale the opener is drawn at. The stylesheet reads the same number from `--ui-scale`. */
  scale?: number
  whileOpen?: MenuWhileOpen
  'aria-label'?: string
}

/** A ContextMenu opened from a point on screen and kept on screen. It closes on a press outside or on an entry's click. */
export function AnchoredMenu({ open = true, ...props }: AnchoredMenuProps) {
  return open ? <OpenAnchoredMenu {...props} /> : null
}

function OpenAnchoredMenu({
  anchor,
  items,
  onClose,
  ignore,
  scale: given,
  whileOpen,
  'aria-label': ariaLabel
}: Omit<AnchoredMenuProps, 'open'>) {
  const root = useRef<HTMLDivElement>(null)
  const closed = useRef(onClose)
  const ignored = useRef(ignore)
  const opened = useRef(whileOpen)
  useEffect(() => {
    closed.current = onClose
    ignored.current = ignore
    opened.current = whileOpen
  })
  // Layout px, before the scale.
  const [size, setSize] = useState({ width: 0, height: 0 })

  useLayoutEffect(() => {
    const node = root.current
    if (node == null) return
    const next = { width: node.offsetWidth, height: node.offsetHeight }
    setSize((current) => (current.width === next.width && current.height === next.height ? current : next))
  })

  useEffect(() => {
    const press = (event: MouseEvent): void => {
      const target = event.target as Node
      if (root.current?.contains(target) === true || ignored.current?.contains(target) === true) return
      closed.current()
    }
    document.addEventListener('mousedown', press)
    const off = opened.current?.(() => closed.current())
    return () => {
      document.removeEventListener('mousedown', press)
      off?.()
    }
  }, [])

  // The focus returns to the opener only if the press that closed the menu did not put it somewhere else.
  useEffect(() => {
    const node = root.current
    const opener = ignored.current ?? document.activeElement
    focusables(node)[0]?.focus()
    return () => {
      const active = document.activeElement
      const lost = active == null || active === document.body || node?.contains(active) === true
      if (lost && opener instanceof HTMLElement && opener.isConnected) opener.focus({ preventScroll: true })
    }
  }, [])

  const scale = given != null && given > 0 ? given : 1
  const { left, top } = anchoredMenuPosition({ x: anchor.x / scale, y: anchor.y / scale }, size, {
    width: window.innerWidth / scale,
    height: window.innerHeight / scale
  })

  const rows = items.map((item): ContextMenuItem => {
    if (!closesMenu(item)) return item
    const run = item.onClick
    return {
      ...item,
      onClick: () => {
        onClose()
        run?.()
      }
    }
  })

  return createPortal(
    <div
      ref={root}
      className={styles.anchored}
      style={{ left: left * scale, top: top * scale }}
      onKeyDown={(event) => {
        if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return
        const list = focusables(root.current)
        const at = list.indexOf(document.activeElement as HTMLElement)
        list[(at + (event.key === 'ArrowDown' ? 1 : list.length - 1)) % list.length]?.focus()
        event.preventDefault()
      }}
    >
      <ContextMenu items={rows} aria-label={ariaLabel} whileSubmenuOpen={whileOpen} />
    </div>,
    document.body
  )
}
