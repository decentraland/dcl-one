import type { ComponentPropsWithoutRef, ElementType, ReactNode } from 'react'
import styles from './NavMenuItem.module.css'

export type NavMenuItemProps<C extends ElementType = 'a'> = {
  as?: C
  label: ReactNode
  className?: string
} & Omit<ComponentPropsWithoutRef<C>, 'as' | 'children' | 'className'>

export function NavMenuItem<C extends ElementType = 'a'>({
  as,
  label,
  className,
  ...rest
}: NavMenuItemProps<C>) {
  const Link: ElementType = as ?? 'a'
  return (
    <Link
      role="menuitem"
      {...rest}
      className={className ? `${styles.root} ${className}` : styles.root}
    >
      {label}
    </Link>
  )
}
