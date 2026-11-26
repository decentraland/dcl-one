import { useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode } from 'react'
import { Switch } from '../Switch/Switch'
import styles from './ContextMenu.module.css'

/**
 * Called as a list opens, with a function that closes it; what it returns runs as the list closes. A host
 * registers the list there with whatever closes its layers (a Cancel action, a gamepad's back button).
 */
export type MenuWhileOpen = (close: () => void) => () => void

export type ContextMenuItem =
  | { kind: 'separator' }
  | { kind: 'title'; label: ReactNode }
  | { kind: 'caption'; label: ReactNode; avatar?: ReactNode }
  | { kind: 'header'; name: ReactNode; tag?: ReactNode; address?: string; avatar?: ReactNode }
  | {
      kind: 'toggle'
      label: ReactNode
      icon?: ReactNode
      checked?: boolean
      onChange?: (checked: boolean) => void
    }
  /** One of a set of exclusive choices (a radio): the checked one carries the mark. */
  | { kind: 'option'; label: ReactNode; icon?: ReactNode; checked: boolean; onClick?: () => void }
  /** A 'submenu' with `items` opens them beside it; without, it only shows its chevron. */
  | {
      kind?: 'item' | 'submenu'
      label: ReactNode
      icon?: ReactNode
      danger?: boolean
      onClick?: () => void
      items?: ContextMenuItem[]
    }

export interface ContextMenuProps {
  items: ContextMenuItem[]
  'aria-label'?: string
  /** Scroll past a long list's maximum height (a submenu's list of names). */
  scroll?: boolean
  /** sheet: a flat sheet with taller rows. */
  variant?: 'default' | 'sheet'
  /** Handed to each list this menu opens beside a row. */
  whileSubmenuOpen?: MenuWhileOpen
}

function shortAddress(address: string): string {
  if (address.length <= 13) return address
  return `${address.slice(0, 6)}…${address.slice(-4)}`
}

function classes(...names: (string | false | undefined)[]): string {
  return names.filter(Boolean).join(' ')
}

/** A stacked menu surface: a header, titles, captions, separators, items, switches, exclusive options and rows that open a list. */
export function ContextMenu({
  items,
  scroll = false,
  variant = 'default',
  whileSubmenuOpen,
  'aria-label': ariaLabel
}: ContextMenuProps) {
  return (
    <div
      className={classes(styles.ctx, scroll && styles.scroll, variant === 'sheet' && styles.sheet)}
      role="menu"
      aria-label={ariaLabel}
    >
      {items.map((item, index) => {
        if (item.kind === 'separator') return <div className={styles.sep} role="separator" key={index} />

        if (item.kind === 'title') {
          return (
            <div className={styles.title} key={index}>
              {item.label}
            </div>
          )
        }

        if (item.kind === 'caption') {
          return (
            <div className={styles.caption} key={index}>
              {item.avatar != null && item.avatar}
              <span className={styles.captionLabel}>{item.label}</span>
            </div>
          )
        }

        if (item.kind === 'header') {
          return (
            <div className={styles.header} key={index}>
              {item.avatar != null && item.avatar}
              <div className={styles.hinfo}>
                <div className={styles.hname}>
                  {item.name}
                  {item.tag != null && <span className={styles.htag}>{item.tag}</span>}
                </div>
                {item.address != null && <span className={styles.wallet}>{shortAddress(item.address)}</span>}
              </div>
            </div>
          )
        }

        if (item.kind === 'toggle') return <ToggleItem item={item} key={index} />

        if (item.kind === 'option') {
          return (
            <button
              type="button"
              className={styles.item}
              role="menuitemradio"
              aria-checked={item.checked}
              key={index}
              onClick={item.onClick}
            >
              {item.icon != null && <span className={styles.icon}>{item.icon}</span>}
              <span className={styles.label}>{item.label}</span>
              <span className={styles.check} aria-hidden="true">
                {item.checked ? <CheckMark /> : null}
              </span>
            </button>
          )
        }

        if (item.kind === 'submenu' && item.items != null) {
          return (
            <Submenu
              key={index}
              label={item.label}
              icon={item.icon}
              className={styles.item}
              whileOpen={whileSubmenuOpen}
            >
              <ContextMenu items={item.items} scroll whileSubmenuOpen={whileSubmenuOpen} />
            </Submenu>
          )
        }

        return (
          <button
            type="button"
            className={classes(styles.item, item.danger && styles.itemDanger)}
            role="menuitem"
            key={index}
            onClick={item.onClick}
          >
            {item.icon != null && <span className={styles.icon}>{item.icon}</span>}
            <span className={styles.label}>{item.label}</span>
            {item.kind === 'submenu' && <span className={styles.chev}>›</span>}
          </button>
        )
      })}
    </div>
  )
}

function CheckMark() {
  return (
    <svg
      viewBox="0 0 12 16"
      width="0.75em"
      height="1em"
      aria-hidden="true"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M1.6 8.6l3 3 5.8-7" />
    </svg>
  )
}

function ToggleItem({ item }: { item: Extract<ContextMenuItem, { kind: 'toggle' }> }) {
  const [on, setOn] = useState(!!item.checked)
  // A new `checked` from the owner wins (a write that failed flips the switch back).
  const [given, setGiven] = useState(item.checked)
  if (item.checked !== given) {
    setGiven(item.checked)
    setOn(!!item.checked)
  }
  const change = (next: boolean): void => {
    setOn(next)
    item.onChange?.(next)
  }
  const name = typeof item.label === 'string' ? item.label : undefined
  return (
    <div
      className={classes(styles.item, styles.itemToggle)}
      role="menuitemcheckbox"
      aria-checked={on}
      aria-label={name}
      onClick={(event) => {
        const label = (event.target as HTMLElement).closest('label')
        if (!label || !event.currentTarget.contains(label)) change(!on)
      }}
    >
      {item.icon != null && <span className={styles.icon}>{item.icon}</span>}
      <span className={styles.label}>{item.label}</span>
      {/* The Switch is a label of its own, so the row is not one: a click beside the switch toggles it. */}
      <Switch appearance="hud" checked={on} onCheckedChange={change} label={name ?? 'Toggle'} labelHidden />
    </div>
  )
}

export interface SubmenuProps {
  label: ReactNode
  icon?: ReactNode
  /** The list it opens, for example `<ContextMenu items={…} scroll />`. */
  children: ReactNode
  /** The host menu's row style, so the row matches its siblings. */
  className?: string
  whileOpen?: MenuWhileOpen
}

/** Where the list stops being visible: the window's edges, or those of the nearest ancestor that clips its overflow. */
function clipEdges(element: HTMLElement): { right: number; bottom: number } {
  let right = window.innerWidth
  let bottom = window.innerHeight
  for (let parent = element.parentElement; parent != null; parent = parent.parentElement) {
    const style = getComputedStyle(parent)
    if (style.overflowX !== 'visible' || style.overflowY !== 'visible') {
      const rect = parent.getBoundingClientRect()
      right = Math.min(right, rect.right)
      bottom = Math.min(bottom, rect.bottom)
      break
    }
  }
  return { right, bottom }
}

/**
 * A menu row that opens a list beside it, on hover or click. A click from the keyboard also moves the focus to
 * the list's first item. The list opens to the right and down, and flips left or up where that side has no room.
 */
export function Submenu({ label, icon, children, className, whileOpen }: SubmenuProps) {
  const [open, setOpen] = useState(false)
  const [flip, setFlip] = useState({ left: false, up: false })
  const [focusFirst, setFocusFirst] = useState(false)
  const flyout = useRef<HTMLDivElement>(null)
  const row = useRef<HTMLButtonElement>(null)
  const listId = useId()
  const opened = useRef(whileOpen)
  useEffect(() => {
    opened.current = whileOpen
  })
  useLayoutEffect(() => {
    if (!open) {
      setFlip({ left: false, up: false })
      return
    }
    const element = flyout.current
    if (element == null) return
    const rect = element.getBoundingClientRect()
    const edges = clipEdges(element)
    setFlip({ left: rect.right > edges.right, up: rect.bottom > edges.bottom })
    if (focusFirst) {
      element
        .querySelector<HTMLElement>('[role^="menuitem"]:not([aria-disabled="true"]):not(:disabled)')
        ?.focus()
      setFocusFirst(false)
    }
  }, [open, focusFirst])
  // The host's close (Cancel) shuts the list first and returns the focus to the row.
  useEffect(() => {
    if (!open) return
    return opened.current?.(() => {
      setOpen(false)
      row.current?.focus()
    })
  }, [open])
  return (
    <div
      className={styles.subWrap}
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setOpen(false)
      }}
    >
      <button
        ref={row}
        type="button"
        className={classes(styles.subRow, className)}
        role="menuitem"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        // A click from the keyboard (detail 0) also focuses the list.
        onClick={(event) => {
          if (event.detail === 0) setFocusFirst(true)
          setOpen(true)
        }}
      >
        {icon != null && <span className={styles.subIcon}>{icon}</span>}
        <span className={styles.subLabel}>{label}</span>
        <span className={styles.subChev} aria-hidden="true">
          ›
        </span>
      </button>
      {open && (
        <div
          id={listId}
          ref={flyout}
          className={classes(styles.flyout, flip.left && styles.flyoutLeft, flip.up && styles.flyoutUp)}
        >
          {children}
        </div>
      )}
    </div>
  )
}
