import type { ReactNode } from 'react'
import styles from './PlacesPage.module.css'

type PlacesPageProps = {
  /** The page heading, visually hidden as source shows none above Live Now. */
  title: string
  /** The Places and Communities tabs, above everything else. */
  tabs?: ReactNode
  /** Live Now, when there is anything live. */
  live?: ReactNode
  /** The Explore band, or the section the tabs select. */
  children: ReactNode
  /** Announcements for assistive technology (results, loading); empty renders nothing visible. */
  status?: string
}

/** The /places page shell: the brand gradient, the padded Live Now area and the full-bleed band. */
function PlacesPage({ title, tabs, live, children, status = '' }: PlacesPageProps) {
  return (
    <main className={styles.page}>
      <h1 className={styles.visuallyHidden}>{title}</h1>
      {tabs ? <div className={styles.tabs}>{tabs}</div> : null}
      {live ? <div className={styles.top}>{live}</div> : null}
      {children}
      <p role="status" className={styles.visuallyHidden}>
        {status}
      </p>
    </main>
  )
}

export { PlacesPage }
export type { PlacesPageProps }
