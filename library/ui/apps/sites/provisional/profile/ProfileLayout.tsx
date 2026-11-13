// TODO(figma-ds): replace with the profile page frame, until the Figma-generated layout from library/ui/components.
import type { ReactNode } from 'react'
import styles from './ProfileLayout.module.css'

type ProfileLayoutProps = {
  /** The desktop header, or the phone's sub-screen header; the phone's section list has none. */
  header?: ReactNode
  tabs?: ReactNode
  /** The avatar column. */
  aside?: ReactNode
  /** Whether the column is open: only Overview shows it, and the phone's section list never does. */
  showAside?: boolean
  children: ReactNode
}

/** The profile page: brand gradient, a centred frame, header, tab bar, and the avatar column beside the body. */
function ProfileLayout({
  header,
  tabs,
  aside,
  showAside = aside != null,
  children
}: ProfileLayoutProps) {
  return (
    <main className={styles.root}>
      <div className={styles.content}>
        <div className={styles.card}>
          {header}
          {tabs ? <div className={styles.tabsArea}>{tabs}</div> : null}
          <div className={styles.split} data-aside={showAside ? 'shown' : 'hidden'}>
            <div className={styles.aside}>{aside}</div>
            <div className={styles.body}>{children}</div>
          </div>
        </div>
      </div>
    </main>
  )
}

export { ProfileLayout }
export type { ProfileLayoutProps }
