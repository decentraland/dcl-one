import type { ButtonHTMLAttributes, ReactNode } from 'react'
import styles from './BarButton.module.css'

export type BarButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  /**
   * `pill` holds the caller's icon and label side by side. `round` is the circle of an icon-only button, which
   * needs an accessible name (`aria-label`).
   */
  shape?: 'pill' | 'round'
  /** `md` is 42px high. `lg` is the 46px circle that closes the page the bar belongs to. */
  size?: 'md' | 'lg'
  /** Keeps the hover background while what the button opens is open. */
  open?: boolean
  /** A small counter on the top right corner, for example the unread notifications. */
  badge?: ReactNode
}

/**
 * A button of the Explorer HUD's top bar: translucent black over a scene or a backdrop. The caller owns the
 * content (an icon, a label, a picture) and what pressing it does; it is a native button.
 */
export function BarButton({
  shape = 'pill',
  size = 'md',
  open = false,
  badge,
  className,
  type = 'button',
  children,
  ...rest
}: BarButtonProps) {
  const classes = [styles.root, styles[shape], size === 'lg' && styles.lg, open && styles.open, className]
    .filter(Boolean)
    .join(' ')
  return (
    <button {...rest} type={type} className={classes}>
      {children}
      {badge != null && badge !== false && <span className={styles.badge}>{badge}</span>}
    </button>
  )
}
