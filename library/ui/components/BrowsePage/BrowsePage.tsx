import type { ReactNode } from 'react'
import { Spinner } from '../Spinner/Spinner'
import styles from './BrowsePage.module.css'

export type BrowseToolbarProps = {
  /** The page's section tabs, at the toolbar's left. */
  tabs: ReactNode
  /** The controls at the toolbar's right, each in a `BrowseControl`. */
  children?: ReactNode
}

/** The toolbar of a browse page: section tabs on the left, search and sort controls on the right. */
export function BrowseToolbar({ tabs, children }: BrowseToolbarProps) {
  return (
    <div className={styles.toolbar}>
      {tabs}
      {children != null && <div className={styles.controls}>{children}</div>}
    </div>
  )
}

export type BrowseControlProps = {
  /** `search` is 240px wide and `select` 168px. */
  size: 'search' | 'select'
  children: ReactNode
}

/** A fixed-width slot in the toolbar's controls, so the row does not reflow as values change. */
export function BrowseControl({ size, children }: BrowseControlProps) {
  return (
    <div className={styles[size]} data-size={size}>
      {children}
    </div>
  )
}

export type BrowsePanelProps = {
  children: ReactNode
  /** Names the panel as a region. */
  label?: string
  /** Marks it loading (`aria-busy`) while the results it shows do not yet answer the current query. */
  busy?: boolean
}

/** The results panel of a browse page. */
export function BrowsePanel({ children, label, busy }: BrowsePanelProps) {
  return (
    <div
      className={styles.panel}
      role={label != null ? 'region' : undefined}
      aria-label={label}
      aria-busy={busy}
    >
      {children}
    </div>
  )
}

/** A centred spinner, for a panel with no results to show yet. */
export function BrowseLoading() {
  return (
    <div className={styles.center}>
      <Spinner variant="thin" size={34} />
    </div>
  )
}
