import type { ComponentPropsWithoutRef, ElementType, ReactNode } from 'react'
import styles from './NavMenuTab.module.css'

export type NavMenuTabProps<C extends ElementType = 'a'> = {
  as?: C
  label: ReactNode
  /** The section the visitor is in: marked as the current page. */
  active?: boolean
  /** The tab opens a menu: it shows the chevron and announces the menu. */
  hasMenu?: boolean
  /** Whether the tab's menu is open; read only when `hasMenu` is set. */
  expanded?: boolean
  className?: string
} & Omit<ComponentPropsWithoutRef<C>, 'as' | 'children' | 'className'>

export function NavMenuTab<C extends ElementType = 'a'>({
  as,
  label,
  active = false,
  hasMenu = false,
  expanded = false,
  className,
  ...rest
}: NavMenuTabProps<C>) {
  const Link: ElementType = as ?? 'a'
  return (
    <Link
      {...rest}
      className={[styles.root, active && styles.active, className].filter(Boolean).join(' ')}
      aria-current={active ? 'page' : undefined}
      aria-haspopup={hasMenu ? 'menu' : undefined}
      aria-expanded={hasMenu ? expanded : undefined}
    >
      {label}
    </Link>
  )
}
