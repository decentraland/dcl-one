// TODO(figma-ds): replace with the Figma-generated profile layout from library/ui/components.
import type { ReactNode } from 'react'
import styles from './ProfileLayout.module.css'

type ProfileLayoutProps = {
  header: ReactNode
  tabs?: ReactNode
  /** The avatar column; shown only on Overview, as in the source. */
  aside?: ReactNode
  children: ReactNode
}

/** The profile page: brand gradient, a centred frame, header, tab bar, and an optional avatar column. */
function ProfileLayout({ header, tabs, aside, children }: ProfileLayoutProps) {
  return (
    <div className={styles.root}>
      <div className={styles.content}>
        {header}
        {tabs ? <div className={styles.tabs}>{tabs}</div> : null}
        <div className={aside ? `${styles.body} ${styles.withAside}` : styles.body}>
          {aside ? <aside className={styles.aside}>{aside}</aside> : null}
          <div className={styles.main}>{children}</div>
        </div>
      </div>
    </div>
  )
}

export { ProfileLayout }
export type { ProfileLayoutProps }
