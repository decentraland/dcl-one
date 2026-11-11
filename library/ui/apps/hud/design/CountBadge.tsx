// CountBadge — a small pill with a count, beside a label or a button's text (pending invites and
// requests, unread items). Hidden from assistive technology: the control it sits in carries the
// count in its accessible name.

import styles from './CountBadge.module.css'

export function CountBadge({ count, max }: { count: number; max?: number }): React.JSX.Element | null {
  if (count <= 0) return null
  return (
    <span className={styles.badge} aria-hidden="true">
      {max != null && count > max ? `+${max}` : count}
    </span>
  )
}
