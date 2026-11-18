import {
  createContext,
  useContext,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode
} from 'react'
import { createPortal } from 'react-dom'
import { Button } from '~/components/Button'
import { MoreHorizIcon } from '~/components/Icons'
import styles from './ActionsMenu.module.css'

type MenuContextValue = { close: () => void }

const MenuContext = createContext<MenuContextValue>({ close: () => {} })

type Props = {
  label: string
  /** Compact 32px trigger for table rows; the default is the page-header icon button. */
  variant?: 'header' | 'row'
  /** `dark` sits the menu on the editor's dark surfaces instead of the violet ones. */
  tone?: 'default' | 'dark'
  /** Trigger test id; the open menu gets `${testId}-menu`. */
  testId: string
  children: ReactNode
}

const VIEWPORT_MARGIN = 8
const GAP = 8

/**
 * The ⋯ dropdown: right-aligned under its trigger, closes on Escape, outside click, or any item click.
 * Portaled to <body> and fixed-positioned, like Select, so a scrolling table never clips or grows for it.
 */
export function ActionsMenu({ label, variant = 'header', tone = 'default', testId, children }: Props) {
  const [isOpen, setOpen] = useState(false)
  const wrapRef = useRef<HTMLDivElement>(null)
  const menuRef = useRef<HTMLDivElement>(null)
  const [menuStyle, setMenuStyle] = useState<CSSProperties>()

  useEffect(() => {
    if (!isOpen) return
    function onMouseDown(event: MouseEvent) {
      const target = event.target as Node
      if (!wrapRef.current?.contains(target) && !menuRef.current?.contains(target)) setOpen(false)
    }
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape') setOpen(false)
    }
    document.addEventListener('mousedown', onMouseDown)
    document.addEventListener('keydown', onKeyDown)
    return () => {
      document.removeEventListener('mousedown', onMouseDown)
      document.removeEventListener('keydown', onKeyDown)
    }
  }, [isOpen])

  // Anchor the menu's right edge to the trigger; open upward when the viewport has no room below.
  useLayoutEffect(() => {
    if (!isOpen) return
    function position() {
      const trigger = wrapRef.current?.getBoundingClientRect()
      const menu = menuRef.current
      if (!trigger || !menu) return
      const fitsBelow = trigger.bottom + GAP + menu.offsetHeight <= window.innerHeight
      const right = Math.max(VIEWPORT_MARGIN, window.innerWidth - trigger.right)
      setMenuStyle({
        right,
        ...(fitsBelow
          ? { top: trigger.bottom + GAP }
          : { top: Math.max(VIEWPORT_MARGIN, trigger.top - GAP - menu.offsetHeight) })
      })
    }
    position()
    window.addEventListener('resize', position)
    window.addEventListener('scroll', position, true)
    return () => {
      window.removeEventListener('resize', position)
      window.removeEventListener('scroll', position, true)
    }
  }, [isOpen])

  return (
    <div className={styles.wrap} ref={wrapRef}>
      <Button
        className={styles.trigger}
        variant={variant === 'row' ? 'secondary' : 'glass'}
        size="icon"
        type="button"
        aria-label={label}
        aria-haspopup="menu"
        aria-expanded={isOpen}
        data-compact={variant === 'row' || undefined}
        data-testid={testId}
        onClick={() => setOpen(open => !open)}
      >
        <MoreHorizIcon fontSize={variant === 'row' ? 'small' : 'medium'} />
      </Button>
      {isOpen &&
        createPortal(
          <MenuContext.Provider value={{ close: () => setOpen(false) }}>
            <div
              className={styles.menu}
              ref={menuRef}
              role="menu"
              style={menuStyle}
              data-tone={tone}
              data-testid={`${testId}-menu`}
            >
              {children}
            </div>
          </MenuContext.Provider>,
          document.body
        )}
    </div>
  )
}

type ItemProps = {
  children: ReactNode
  onClick?: () => void
  /** Inert placeholder for a flow that has not shipped yet; `title` carries the "coming soon" hint. */
  disabled?: boolean
  title?: string
  /** `danger` for a destructive entry (delete, transfer ownership). */
  tone?: 'default' | 'danger'
  testId: string
}

export function ActionsMenuItem({ children, onClick, disabled = false, title, tone = 'default', testId }: ItemProps) {
  const { close } = useContext(MenuContext)
  return (
    <button
      className={styles.item}
      type="button"
      role="menuitem"
      aria-disabled={disabled || undefined}
      title={title}
      data-tone={tone === 'danger' ? tone : undefined}
      data-testid={testId}
      onClick={() => {
        if (disabled) return
        close()
        onClick?.()
      }}
    >
      {children}
    </button>
  )
}

export function ActionsMenuDivider() {
  return <div className={styles.divider} />
}
