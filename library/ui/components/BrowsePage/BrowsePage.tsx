import type { ReactNode } from 'react'
import { Spinner } from '../Spinner/Spinner'
import styles from './BrowsePage.module.css'

function classes(...names: (string | undefined)[]): string {
  return names.filter(Boolean).join(' ')
}

export type BrowseToolbarProps = {
  /** The page's section tabs, at the toolbar's left. */
  tabs: ReactNode
  /** The controls at the toolbar's right, each in a `BrowseControl`. */
  children?: ReactNode
  className?: string
}

/** The toolbar of a browse page: section tabs on the left, search and sort controls on the right. */
export function BrowseToolbar({ tabs, children, className }: BrowseToolbarProps) {
  return (
    <div className={classes(styles.toolbar, className)}>
      {tabs}
      {children != null && <div className={styles.controls}>{children}</div>}
    </div>
  )
}

export type BrowseControlProps = {
  /** `search` is 240px wide and `select` 168px. */
  size: 'search' | 'select'
  children: ReactNode
  className?: string
}

/** A fixed-width slot in the toolbar's controls, so the row does not reflow as values change. */
export function BrowseControl({ size, children, className }: BrowseControlProps) {
  return (
    <div className={classes(styles[size], className)} data-size={size}>
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
  className?: string
}

/** The results panel of a browse page. */
export function BrowsePanel({ children, label, busy, className }: BrowsePanelProps) {
  return (
    <div
      className={classes(styles.panel, className)}
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
