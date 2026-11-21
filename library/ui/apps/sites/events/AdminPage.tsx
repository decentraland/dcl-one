import type { ReactNode } from 'react'
import styles from './AdminPage.module.css'

/**
 * decentraland.org's AdminLayout: the radial purple page the event administration pages share, a solid
 * dark violet on phones. `bar` (the admin tabs) sits on top of it, under the site header.
 */
export function AdminPage({ bar, children }: { bar?: ReactNode; children: ReactNode }) {
  return (
    <div className={styles.root}>
      {bar}
      <main className={styles.content}>{children}</main>
    </div>
  )
}

export const adminPageStyles = styles
