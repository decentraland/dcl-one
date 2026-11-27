import {
  createContext,
  useContext,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode
} from 'react'
import { GeneralCheck, GeneralRightArrow } from '../../foundations/icons/general'
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
  /** One of a set of exclusive choices (a radio): the checked one carries the mark where a row's icon sits. */
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
  /** The surface's width in px. Unset, it fits its rows, from 180 to 280px. */
  width?: number
  /** Handed to each list this menu opens beside a row. */
  whileSubmenuOpen?: MenuWhileOpen
}

/** Whether a row sits in a ContextMenu: a Submenu row there takes the menu's icon box and chevron. */
const InMenu = createContext(false)

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
  width,
  whileSubmenuOpen,
  'aria-label': ariaLabel
}: ContextMenuProps) {
  return (
    <InMenu.Provider value={true}>
      <div
        className={classes(styles.ctx, scroll && styles.scroll)}
        style={width != null ? ({ '--menu-width': `${width}px` } as CSSProperties) : undefined}
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
                <span className={styles.icon} aria-hidden="true">
                  {item.checked ? <GeneralCheck size={20} /> : null}
                </span>
                {item.icon != null && <span className={styles.icon}>{item.icon}</span>}
                <span className={styles.label}>{item.label}</span>
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
              {item.kind === 'submenu' && <Chevron />}
            </button>
          )
        })}
      </div>
    </InMenu.Provider>
  )
}

function Chevron() {
  return (
    <span className={styles.chev} aria-hidden="true">
      <GeneralRightArrow size={20} />
    </span>
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
      className={styles.item}
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
 * the list's first item; a pointer click leaves the focus on neither, so a later key press (in the Explorer HUD, the
 * game's) rings nothing. The list opens to the right and down, and flips left or up where that side has no room.
 */
export function Submenu({ label, icon, children, className, whileOpen }: SubmenuProps) {
  const inMenu = useContext(InMenu)
  const [open, setOpen] = useState(false)
  // Null until the open list is measured.
  const [flip, setFlip] = useState<{ left: boolean; up: boolean } | null>(null)
  const [focusFirst, setFocusFirst] = useState(false)
  const flyout = useRef<HTMLDivElement>(null)
  const row = useRef<HTMLButtonElement>(null)
  // Whether the keyboard opened the list: only then does closing it return the focus to the row.
  const fromKeyboard = useRef(false)
  const listId = useId()
  const opened = useRef(whileOpen)
  useEffect(() => {
    opened.current = whileOpen
  })
  // Measured once per opening: measured again once flipped, the list would find room and flip back.
  useLayoutEffect(() => {
    if (!open) {
      setFlip(null)
      return
    }
    const element = flyout.current
    if (element == null) return
    const rect = element.getBoundingClientRect()
    const edges = clipEdges(element)
    setFlip({ left: rect.right > edges.right, up: rect.bottom > edges.bottom })
  }, [open])
  // Once the list is placed, so the focus does not scroll a clipping ancestor towards where it would have been.
  useLayoutEffect(() => {
    if (flip == null || !focusFirst) return
    flyout.current
      ?.querySelector<HTMLElement>('[role^="menuitem"]:not([aria-disabled="true"]):not(:disabled)')
      ?.focus()
    setFocusFirst(false)
  }, [flip, focusFirst])
  // The host's close (Cancel) shuts the list first, and returns the focus to the row if the keyboard opened it.
  useEffect(() => {
    if (!open) return
    return opened.current?.(() => {
      setOpen(false)
      if (fromKeyboard.current) row.current?.focus()
    })
  }, [open])
  return (
    <div
      className={styles.subWrap}
      onMouseEnter={() => {
        if (!open) fromKeyboard.current = false
        setOpen(true)
      }}
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
        // A click from the keyboard (detail 0) also focuses the list; a pointer click drops the row's focus.
        onClick={(event) => {
          fromKeyboard.current = event.detail === 0
          if (fromKeyboard.current) setFocusFirst(true)
          else event.currentTarget.blur()
          setOpen(true)
        }}
      >
        {icon != null && <span className={inMenu ? styles.icon : styles.subIcon}>{icon}</span>}
        <span className={styles.label}>{label}</span>
        {inMenu ? (
          <Chevron />
        ) : (
          <span className={styles.subChev} aria-hidden="true">
            ›
          </span>
        )}
      </button>
      {open && (
        <div
          id={listId}
          ref={flyout}
          className={classes(styles.flyout, flip?.left && styles.flyoutLeft, flip?.up && styles.flyoutUp)}
        >
          {children}
        </div>
      )}
    </div>
  )
}
