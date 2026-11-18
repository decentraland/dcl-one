import { cx } from '@/data/lib/creators/cx'
import styles from './ErrorNotice.module.css'
import { SaleIcon } from './SaleIcon'

/** An inline error, or nothing without a message; announced when it appears. */
export function ErrorNotice({
  message,
  className,
  testId
}: {
  message?: string | null
  className?: string
  testId?: string
}) {
  if (!message) return null
  return (
    <p className={cx(styles.notice, className)} data-testid={testId} role="alert">
      <SaleIcon name="alert" size={18} />
      <span className={styles.message}>{message}</span>
    </p>
  )
}
