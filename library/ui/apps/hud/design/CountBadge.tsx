// CountBadge — a small pill with a count, beside a label or a button's text (pending invites and
// requests, unread items, a tab's count). Hidden from assistive technology unless `announce`: the
// control it sits in then carries the count in its accessible name instead.

import styles from './CountBadge.module.css'

export function CountBadge({
  count,
  max,
  size = 'md',
  announce = false
}: {
  count: number
  /** Above it, the badge shows `+max`. */
  max?: number
  size?: 'xs' | 'sm' | 'md'
  /** Read the count as part of the control's name (a tab's count). */
  announce?: boolean
}): React.JSX.Element | null {
  if (count <= 0) return null
  return (
    <span className={`${styles.badge} ${styles[size]}`} aria-hidden={announce ? undefined : 'true'}>
      {max != null && count > max ? `+${max}` : count}
    </span>
  )
}
