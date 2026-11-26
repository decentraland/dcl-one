import styles from './CountBadge.module.css'

export type CountBadgeProps = {
  /** Nothing renders at zero or below. */
  count: number
  /** Above it, the badge shows `+max`. */
  max?: number
  /** `md` is 20px high; `sm` (18px) sits in a tab and `xs` (16px) in a tab of an equal-width strip. */
  size?: 'xs' | 'sm' | 'md'
  /**
   * Lets assistive technology read the count as part of the control it sits in (a tab's count).
   * Without it the badge is hidden, and the control carries the count in its own accessible name.
   */
  announce?: boolean
}

/**
 * A small ruby pill with a count, beside a label or a button's text: pending requests, unread items,
 * a tab's count.
 */
export function CountBadge({ count, max, size = 'md', announce = false }: CountBadgeProps) {
  if (!(count > 0)) return null
  return (
    <span className={`${styles.root} ${styles[size]}`} aria-hidden={announce ? undefined : 'true'}>
      {max != null && count > max ? `+${max}` : count}
    </span>
  )
}
