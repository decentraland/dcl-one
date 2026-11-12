// TODO(figma-ds): replace with the Figma-generated empty message from library/ui/components.
import type { ReactNode } from 'react'
import styles from './EmptyState.module.css'

type EmptyStateProps = {
  /** The glyph inside the rounded frame; decorative. */
  icon: ReactNode
  title: string
  subtitle?: string
  /** The next step, usually a Button; shown under the text. */
  action?: ReactNode
}

/** A centred icon frame, a title, an optional subtitle and an optional call to action. */
function EmptyState({ icon, title, subtitle, action }: EmptyStateProps) {
  return (
    <div className={styles.root}>
      <span className={styles.icon} aria-hidden="true">
        {icon}
      </span>
      <div className={styles.body}>
        <p className={styles.title}>{title}</p>
        {subtitle ? <p className={styles.subtitle}>{subtitle}</p> : null}
        {action ? <div className={styles.action}>{action}</div> : null}
      </div>
    </div>
  )
}

export { EmptyState }
export type { EmptyStateProps }
