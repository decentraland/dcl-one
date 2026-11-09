// unity-explorer's GenericContextMenu, ported one to one for the menus that port a Unity one (the
// camera-reel photo options); the HUD's own menus use ContextMenu.

import { useEffect, useRef } from 'react'
import { createPortal } from 'react-dom'
import { getHudScale } from '../lib/hudScale'
import { registerCancelLayer } from '../lib/cancelLayers'
import { MaskIcon } from './MaskIcon'
import styles from './GenericContextMenu.module.css'

export type GenericContextMenuItem =
  | { kind: 'toggle'; label: string; checked: boolean; onChange: (checked: boolean) => void }
  | { kind: 'separator' }
  | { kind: 'button'; label: string; icon: string; onClick: () => void }

// Unity's defaults (GenericContextMenu.cs, the control settings and their prefabs), canvas px.
const WIDTH = 186
const PADDING_TOP = 4
const PADDING_BOTTOM = 12
const SPACING = 1
const OFFSET_X = 11
const OFFSET_Y = 18
const ROW_HEIGHT: Record<GenericContextMenuItem['kind'], number> = { toggle: 34, separator: 8, button: 36 }

export function genericContextMenuHeight(items: GenericContextMenuItem[]): number {
  const rows = items.reduce((sum, item) => sum + ROW_HEIGHT[item.kind], 0)
  return rows + PADDING_TOP + PADDING_BOTTOM + SPACING * Math.max(0, items.length - 1)
}

/** Unity's GetControlsPosition for BottomRight, with its flip and clamp. Unity's y axis points up,
 *  so its +18 lifts the menu: the top edge sits 18 above the anchor. */
export function genericContextMenuPosition(
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

export function GenericContextMenu({
  anchor,
  items,
  onClose,
  ignore,
  'aria-label': ariaLabel
}: {
  /** The point the menu opens from, in viewport px (Unity anchors on the opener's centre). */
  anchor: { x: number; y: number }
  items: GenericContextMenuItem[]
  onClose: () => void
  /** An element whose presses don't count as "outside" (the button that toggles the menu). */
  ignore?: Element | null
  'aria-label'?: string
}): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null)
  const onCloseRef = useRef(onClose)
  onCloseRef.current = onClose
  const ignoreRef = useRef(ignore)
  ignoreRef.current = ignore

  useEffect(() => {
    const onDown = (e: MouseEvent): void => {
      const target = e.target as Node
      if (ref.current?.contains(target) === true || ignoreRef.current?.contains(target) === true) return
      onCloseRef.current()
    }
    document.addEventListener('mousedown', onDown)
    const off = registerCancelLayer(() => onCloseRef.current())
    return () => {
      document.removeEventListener('mousedown', onDown)
      off()
    }
  }, [])

  // Focus returns to the opener only if the press that closed the menu didn't put it somewhere else.
  useEffect(() => {
    const node = ref.current
    const opener = ignoreRef.current ?? document.activeElement
    node?.querySelector<HTMLElement>('[role^="menuitem"]')?.focus()
    return () => {
      const active = document.activeElement
      const lost = active == null || active === document.body || node?.contains(active) === true
      if (lost && opener instanceof HTMLElement && opener.isConnected) opener.focus({ preventScroll: true })
    }
  }, [])

  const scale = getHudScale() || 1
  const { left, top } = genericContextMenuPosition(
    { x: anchor.x / scale, y: anchor.y / scale },
    { width: WIDTH, height: genericContextMenuHeight(items) },
    { width: window.innerWidth / scale, height: window.innerHeight / scale }
  )

  return createPortal(
    <div
      ref={ref}
      role="menu"
      aria-label={ariaLabel}
      className={styles.menu}
      style={{ left: left * scale, top: top * scale }}
      onKeyDown={(e) => {
        if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return
        const rows = Array.from(ref.current?.querySelectorAll<HTMLElement>('[role^="menuitem"]') ?? [])
        const at = rows.indexOf(document.activeElement as HTMLElement)
        rows[(at + (e.key === 'ArrowDown' ? 1 : rows.length - 1)) % rows.length]?.focus()
        e.preventDefault()
      }}
    >
      {items.map((item, i) => {
        if (item.kind === 'separator') {
          return (
            <div key={i} role="separator" className={styles.separator}>
              <span className={styles.line} />
            </div>
          )
        }
        if (item.kind === 'toggle') {
          return (
            <div key={i} className={styles.toggleRow}>
              <span className={styles.label}>{item.label}</span>
              <button
                type="button"
                role="menuitemcheckbox"
                aria-checked={item.checked}
                aria-label={item.label}
                className={`${styles.switch} ${item.checked ? styles.on : ''}`.trim()}
                onClick={() => item.onChange(!item.checked)}
              >
                <span className={styles.knob} />
              </button>
            </div>
          )
        }
        return (
          <button
            key={i}
            type="button"
            role="menuitem"
            className={styles.button}
            onClick={() => {
              onClose()
              item.onClick()
            }}
          >
            <MaskIcon src={item.icon} size={22} />
            <span className={styles.label}>{item.label}</span>
          </button>
        )
      })}
    </div>,
    document.body
  )
}
