import type { ComponentPropsWithoutRef, ReactNode } from 'react'
import { Button } from '../../components/Button/Button'
import styles from './ShopEmptyState.module.css'

type Action = { label: string } & (
  { href: string; onClick?: never } | { href?: never; onClick: () => void }
)

type ShopEmptyStateProps = Omit<ComponentPropsWithoutRef<'div'>, 'title' | 'children'> & {
  title: string
  subtitle?: string
  icon?: ReactNode
  variant?: 'inline' | 'screen'
  action?: Action
}

export function ShopEmptyState({
  title,
  subtitle,
  icon,
  variant,
  action,
  className,
  ...rest
}: ShopEmptyStateProps) {
  return (
    <div
      {...rest}
      className={[styles.root, className].filter(Boolean).join(' ')}
      data-variant={variant}
    >
      {icon != null && (
        <div className={styles.icon} aria-hidden="true">
          {icon}
        </div>
      )}
      <h2 className={styles.title}>{title}</h2>
      {subtitle && <p className={styles.subtitle}>{subtitle}</p>}
      {action && (
        <div className={styles.action}>
          {action.href !== undefined ? (
            <Button size="md" href={action.href}>
              {action.label}
            </Button>
          ) : (
            <Button size="md" onClick={action.onClick}>
              {action.label}
            </Button>
          )}
        </div>
      )}
    </div>
  )
}
